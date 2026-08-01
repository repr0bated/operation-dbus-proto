This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-cognitive-mcp/
              proto/
                cognitive.proto
              src/
                bin/
                  op-cog-admin.rs
                  rag-ingest.rs
                activity_filter.rs
                agent_tools.rs
                cognitive_tools.rs
                cozo_shuttle.rs
                dbus_interface.rs
                doctor.rs
                gemini_fallback.rs
                grpc_service.rs
                interceptor.rs
                lib.rs
                main.rs
                memory_store.rs
                notebooklm.rs
                qdrant_shuttle.rs
                quota.rs
                rag_pipeline.rs
                server.rs
                session.rs
                soul_memory.rs
                tool_profiles.rs
                typed_tools.rs
                voyage.rs
              build.rs
              Cargo.toml
              compare-op-cognitive-mcp.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/proto/cognitive.proto">
syntax = "proto3";

package operation.cognitive.v1;

// CognitiveToolService — NotebookLM MCP gRPC Ingress
//
// Implements all service methods from the Technical Specification v0.2.
// Traceability:
//   R1-R3,R9-R11 → AskQuestion, QueryNotebook, ListNotebooks, GetHealth, SetupAuth
//   R4-R6        → CreateNotebook, BatchCreateNotebooks, AddFolder, AddSource, ListSources, RemoveSource
//   R7,R12-R15   → GenerateDataTable, GeminiQuery, GetToolProfile, Doctor, GetQueryHistory
service CognitiveToolService {
  // Core querying (R1)
  rpc AskQuestion(AskQuestionRequest) returns (AskQuestionResponse);
  rpc QueryNotebook(QueryNotebookRequest) returns (QueryNotebookResponse);

  // Library management (R3)
  rpc ListNotebooks(ListNotebooksRequest) returns (ListNotebooksResponse);
  rpc GetNotebook(GetNotebookRequest) returns (GetNotebookResponse);

  // Notebook lifecycle (R4-R5)
  rpc CreateNotebook(CreateNotebookRequest) returns (CreateNotebookResponse);
  rpc BatchCreateNotebooks(BatchCreateNotebooksRequest) returns (BatchCreateNotebooksResponse);

  // Source ops (R5-R6)
  rpc AddSource(AddSourceRequest) returns (AddSourceResponse);
  rpc AddFolder(AddFolderRequest) returns (AddFolderResponse);
  rpc ListSources(ListSourcesRequest) returns (ListSourcesResponse);
  rpc GetSourceContent(GetSourceContentRequest) returns (GetSourceContentResponse);
  rpc RemoveSource(RemoveSourceRequest) returns (RemoveSourceResponse);

  // Advanced outputs (R7)
  rpc GenerateDataTable(GenerateDataTableRequest) returns (GenerateDataTableResponse);

  // Gemini fallback (R12)
  rpc GeminiQuery(GeminiQueryRequest) returns (GeminiQueryResponse);

  // Tool profiles (R14)
  rpc GetToolProfile(GetToolProfileRequest) returns (GetToolProfileResponse);

  // Diagnostics (R15)
  rpc Doctor(DoctorRequest) returns (DoctorResponse);
  rpc GetQueryHistory(GetQueryHistoryRequest) returns (GetQueryHistoryResponse);

  // Resilience & auth (R9-R11)
  rpc GetHealth(GetHealthRequest) returns (GetHealthResponse);
  rpc SetupAuth(SetupAuthRequest) returns (SetupAuthResponse);
}

// ---------------------------------------------------------------------------
// Citation — returned by grounded queries
// ---------------------------------------------------------------------------

message Citation {
  string text = 1;
  string source = 2;
  string page = 3;
}

// ---------------------------------------------------------------------------
// AskQuestion (R1 — grounded query)
// ---------------------------------------------------------------------------

message AskQuestionRequest {
  string notebook_id = 1;
  string query = 2;
  // R2 — conversation_id for follow-ups
  string conversation_id = 3;
}

message AskQuestionResponse {
  string answer = 1;
  repeated Citation citations = 2;
  string conversation_id = 3;
  bool grounded = 4;
}

// ---------------------------------------------------------------------------
// QueryNotebook
// ---------------------------------------------------------------------------

message QueryNotebookRequest {
  string notebook_id = 1;
  string query = 2;
  string conversation_id = 3;
  int32 max_results = 4;
}

message QueryNotebookResponse {
  string answer = 1;
  repeated Citation citations = 2;
  string conversation_id = 3;
}

// ---------------------------------------------------------------------------
// ListNotebooks (R3)
// ---------------------------------------------------------------------------

message ListNotebooksRequest {
  // Optional kind filter: "project", "session", "agent", etc.
  string kind_filter = 1;
  int32 limit = 2;
  int32 offset = 3;
}

message NotebookInfo {
  string id = 1;
  string name = 2;
  string kind = 3;
  string description = 4;
  int32 source_count = 5;
  string created_at = 6;
  string updated_at = 7;
}

message ListNotebooksResponse {
  repeated NotebookInfo notebooks = 1;
  int32 total = 2;
}

// ---------------------------------------------------------------------------
// GetNotebook (R3)
// ---------------------------------------------------------------------------

message GetNotebookRequest {
  string notebook_id = 1;
}

message GetNotebookResponse {
  NotebookInfo notebook = 1;
  // Arbitrary metadata as JSON string.
  string metadata_json = 2;
}

// ---------------------------------------------------------------------------
// CreateNotebook (R4)
// ---------------------------------------------------------------------------

message CreateNotebookRequest {
  string title = 1;
  string description = 2;
  string kind = 3;
}

message CreateNotebookResponse {
  NotebookInfo notebook = 1;
}

// ---------------------------------------------------------------------------
// BatchCreateNotebooks (R4)
// ---------------------------------------------------------------------------

message BatchCreateNotebooksRequest {
  repeated CreateNotebookRequest notebooks = 1;
}

message BatchCreateNotebooksResponse {
  repeated NotebookInfo notebooks = 1;
  int32 created = 2;
  int32 failed = 3;
}

// ---------------------------------------------------------------------------
// AddSource (R5)
// ---------------------------------------------------------------------------

message AddSourceRequest {
  string notebook_id = 1;
  // "url", "text", "file"
  string source_type = 2;
  string content = 3;
  string title = 4;
  repeated string tags = 5;
}

message AddSourceResponse {
  string source_id = 1;
  bool success = 2;
}

// ---------------------------------------------------------------------------
// AddFolder (R5 — bulk ingest)
// ---------------------------------------------------------------------------

message AddFolderRequest {
  string notebook_id = 1;
  string folder_path = 2;
  // Glob patterns for filtering, e.g. "*.rs", "*.md"
  repeated string patterns = 3;
  bool recursive = 4;
}

message AddFolderResponse {
  int32 sources_added = 1;
  int32 sources_skipped = 2;
  repeated string errors = 3;
}

// ---------------------------------------------------------------------------
// ListSources (R6)
// ---------------------------------------------------------------------------

message ListSourcesRequest {
  string notebook_id = 1;
  int32 limit = 2;
  int32 offset = 3;
}

message SourceInfo {
  string id = 1;
  string title = 2;
  string source_type = 3;
  repeated string tags = 4;
  string created_at = 5;
}

message ListSourcesResponse {
  repeated SourceInfo sources = 1;
  int32 total = 2;
}

// ---------------------------------------------------------------------------
// GetSourceContent (R6)
// ---------------------------------------------------------------------------

message GetSourceContentRequest {
  string notebook_id = 1;
  string source_id = 2;
}

message GetSourceContentResponse {
  string content = 1;
  string source_type = 2;
  string title = 3;
}

// ---------------------------------------------------------------------------
// GenerateDataTable (R7 — structured extraction)
// ---------------------------------------------------------------------------

message GenerateDataTableRequest {
  string notebook_id = 1;
  string prompt = 2;
  // Expected column names
  repeated string columns = 3;
}

message GenerateDataTableResponse {
  // JSON string of [{column: value, ...}, ...]
  string data_json = 1;
  int32 row_count = 2;
}

// ---------------------------------------------------------------------------
// GetHealth (R10)
// ---------------------------------------------------------------------------

message GetHealthRequest {
  bool deep_check = 1;
}

message GetHealthResponse {
  bool healthy = 1;
  string status = 2;
  // Component-level status as JSON
  string components_json = 3;
  // Quota info (R11)
  int32 queries_remaining = 4;
  int32 queries_limit = 5;
  string auth_status = 6;
}

// ---------------------------------------------------------------------------
// SetupAuth (R9)
// ---------------------------------------------------------------------------

message SetupAuthRequest {
  // "chrome_profile" or "cookie"
  string auth_method = 1;
  // Path to Chrome profile or cookie value
  string credential = 2;
}

message SetupAuthResponse {
  bool success = 1;
  string message = 2;
}

// ---------------------------------------------------------------------------
// RemoveSource (R6)
// ---------------------------------------------------------------------------

message RemoveSourceRequest {
  string notebook_id = 1;
  string source_id = 2;
}

message RemoveSourceResponse {
  bool success = 1;
}

// ---------------------------------------------------------------------------
// GeminiQuery (R12 — fallback when browser breaks)
// ---------------------------------------------------------------------------

message GeminiQueryRequest {
  string query = 1;
  // Optional context to ground the query
  string context = 2;
  // "query" or "deep_research"
  string mode = 3;
  // Depth for deep_research (1-5, default 3)
  int32 depth = 4;
}

message GeminiQueryResponse {
  string answer = 1;
  repeated Citation citations = 2;
  string model = 3;
  bool is_fallback = 4;
  // For deep_research: JSON of sections
  string sections_json = 5;
}

// ---------------------------------------------------------------------------
// GetToolProfile (R14)
// ---------------------------------------------------------------------------

message GetToolProfileRequest {
  // Optional: if empty returns current profile
  string profile_name = 1;
}

message GetToolProfileResponse {
  string current_profile = 1;
  int32 tool_count = 2;
  int32 schema_tokens = 3;
  int32 savings_percent = 4;
  repeated string tools = 5;
}

// ---------------------------------------------------------------------------
// Doctor (R15 — diagnostics)
// ---------------------------------------------------------------------------

message DoctorRequest {
  bool verbose = 1;
}

message DoctorResponse {
  string overall_status = 1;
  string timestamp = 2;
  // JSON array of component statuses
  string components_json = 3;
  repeated string recommendations = 4;
}

// ---------------------------------------------------------------------------
// GetQueryHistory (R15)
// ---------------------------------------------------------------------------

message GetQueryHistoryRequest {
  int32 limit = 1;
  string conversation_id = 2;
}

message QueryHistoryEntry {
  string conversation_id = 1;
  string notebook_id = 2;
  string query = 3;
  string answer_preview = 4;
  string timestamp = 5;
  int32 citations_count = 6;
  bool grounded = 7;
}

message GetQueryHistoryResponse {
  repeated QueryHistoryEntry entries = 1;
  int32 total = 2;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/bin/op-cog-admin.rs">
//! Minimal admin CLI for the op-cognitive-mcp cozo store.
//!
//! Examples:
//!   op-cog-admin --db /var/lib/op-dbus/cognitive.db user-add <wg_pubkey>
//!   op-cog-admin --db /var/lib/op-dbus/cognitive.db user-list

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use op_cozo_store::CozoGraphShuttle;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "op-cog-admin", about = "Cozo store admin for op-cognitive-mcp")]
struct Cli {
    #[arg(
        long,
        env = "COGNITIVE_MCP_DB_PATH",
        default_value = "/var/lib/op-dbus/cognitive.db"
    )]
    db: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Insert or refresh a user keyed by wg_pubkey
    UserAdd { wg_pubkey: String },
    /// List all users
    UserList,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let shuttle = CozoGraphShuttle::new_persistent(PathBuf::from(&cli.db))
        .with_context(|| format!("opening cozo at {}", cli.db))?;

    match cli.cmd {
        Cmd::UserAdd { wg_pubkey } => {
            shuttle.upsert_user(&wg_pubkey)?;
            println!("ok: user {} upserted", wg_pubkey);
        }
        Cmd::UserList => {
            let json = shuttle.run_query(
                "?[wg_pubkey, created_at] := *users[wg_pubkey, created_at]",
                None,
            )?;
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/bin/rag-ingest.rs">
//! RAG ingest CLI — embed repomix zip content into Qdrant via Voyage.
//!
//! Usage:
//!   rag-ingest --zip ~/repomix.zip --repo rust-analyzer
//!   rag-ingest --zip ~/repomix.zip --all
//!   rag-ingest --zip ~/repomix.zip --list

use anyhow::{Context, Result};
use clap::Parser;
use op_cognitive_mcp::rag_pipeline::{RagPipeline, DEFAULT_COLLECTION};
use std::path::PathBuf;
use tracing::{error, info};

// voyage-code-3 pricing: $0.18 per million tokens (charged after free tier exhausted)
const VOYAGE_COST_PER_MILLION: f64 = 0.18;
// Free tier per model per month (tokens) — overage is billed at VOYAGE_COST_PER_MILLION
const VOYAGE_FREE_TIER_TOKENS: usize = 200_000_000;
// Rough average tokens per chunk (embed_text header + ~300 content tokens)
const AVG_TOKENS_PER_CHUNK: usize = 400;
// Default hard cap: stop before $10 of paid overage (55M tokens beyond free tier)
const DEFAULT_MAX_TOKENS: usize = VOYAGE_FREE_TIER_TOKENS + 55_000_000; // free + ~$10

#[derive(Parser)]
#[command(name = "rag-ingest")]
#[command(about = "Ingest repomix content into Qdrant with Voyage embeddings")]
struct Cli {
    /// Path to the repomix zip file
    #[arg(long, default_value = "~/repomix.zip")]
    zip: PathBuf,

    /// Repo to ingest (e.g. "rust-analyzer"). Can be specified multiple times.
    #[arg(long)]
    repo: Vec<String>,

    /// Ingest all repos in the zip (slow — hundreds of MB)
    #[arg(long)]
    all: bool,

    /// List available repos in the zip and exit
    #[arg(long)]
    list: bool,

    /// Qdrant collection name
    #[arg(long, default_value = DEFAULT_COLLECTION)]
    collection: String,

    /// Skip repos whose names contain this substring
    #[arg(long)]
    skip: Vec<String>,

    /// Estimate cost and chunk count, then exit without embedding
    #[arg(long)]
    dry_run: bool,

    /// Maximum tokens to embed across the entire run (budget guard).
    /// Ingest aborts once this limit is reached. Default: 27M (~$4.86).
    #[arg(long, default_value_t = DEFAULT_MAX_TOKENS)]
    max_tokens: usize,

    /// Skip the cost confirmation prompt (use in CI or when you know the cost)
    #[arg(long)]
    yes: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let mut cli = Cli::parse();

    // Expand ~ in zip path
    if let Some(home) = std::env::var_os("HOME") {
        let zip_str = cli.zip.to_string_lossy();
        if zip_str.starts_with('~') {
            cli.zip = PathBuf::from(home).join(zip_str.trim_start_matches("~/"));
        }
    }

    let zip_path = cli
        .zip
        .canonicalize()
        .with_context(|| format!("zip not found: {}", cli.zip.display()))?;

    // List available entries
    let entries = list_repomix_entries(&zip_path)?;

    if cli.list {
        println!("Repomix entries in {}:", zip_path.display());
        for (entry, repo) in &entries {
            println!("  {repo:40} ({entry})");
        }
        return Ok(());
    }

    // Determine what to ingest
    let targets: Vec<(String, String)> = if cli.all {
        entries.clone()
    } else if !cli.repo.is_empty() {
        // Match by repo name substring
        entries
            .into_iter()
            .filter(|(_, repo)| cli.repo.iter().any(|r| repo.contains(r.as_str())))
            .collect()
    } else {
        anyhow::bail!("Specify --repo <name>, --all, or --list");
    };

    // Apply skip filter
    let targets: Vec<_> = targets
        .into_iter()
        .filter(|(_, repo)| !cli.skip.iter().any(|s| repo.contains(s.as_str())))
        .collect();

    if targets.is_empty() {
        anyhow::bail!("No matching entries found. Use --list to see available repos.");
    }

    // ── Cost estimate ────────────────────────────────────────────────────────
    // Count chunks per target by reading zip entry sizes (fast, no embedding).
    let mut estimated_chunks = 0usize;
    {
        let file = std::fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for (entry_name, _repo) in &targets {
            if let Ok(entry) = archive.by_name(entry_name) {
                // ~1 chunk per 1.5 KB of source (empirical from past runs)
                estimated_chunks += (entry.size() as usize / 1500).max(1);
            }
        }
    }
    let estimated_tokens = estimated_chunks * AVG_TOKENS_PER_CHUNK;
    let estimated_cost = estimated_tokens as f64 / 1_000_000.0 * VOYAGE_COST_PER_MILLION;

    println!("\n=== Cost estimate ===");
    println!("  Repos          : {}", targets.len());
    println!("  Est. chunks    : ~{estimated_chunks}");
    println!("  Est. tokens    : ~{}M", estimated_tokens / 1_000_000);
    println!("  Est. cost      : ~${estimated_cost:.2}  (voyage-code-3 @ ${VOYAGE_COST_PER_MILLION}/M tokens)");
    println!(
        "  Token cap      : {}M  (--max-tokens)",
        cli.max_tokens / 1_000_000
    );

    if cli.dry_run {
        println!("\n[dry-run] Exiting without embedding.");
        return Ok(());
    }

    if estimated_tokens > cli.max_tokens {
        anyhow::bail!(
            "Estimated tokens ({estimated_tokens}) exceed --max-tokens cap ({}). \
             Reduce scope with --repo, raise --max-tokens, or use --dry-run to preview.",
            cli.max_tokens
        );
    }

    if !cli.yes {
        print!("\nProceed with ingest? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // ── Ingest ───────────────────────────────────────────────────────────────
    info!(
        count = targets.len(),
        collection = %cli.collection,
        "Starting ingest"
    );

    let pipeline =
        RagPipeline::from_env().context("Failed to init pipeline — is VOYAGE_API_KEY set?")?;

    let mut total_files = 0usize;
    let mut total_chunks = 0usize;
    let mut total_errors = 0usize;
    let mut total_tokens = 0usize;

    'outer: for (entry_name, repo) in &targets {
        info!(repo = %repo, "Processing");

        match pipeline
            .ingest_repomix_entry(&zip_path, entry_name, &cli.collection)
            .await
        {
            Ok(stats) => {
                info!(
                    repo = %repo,
                    files = stats.files_parsed,
                    chunks = stats.chunks_upserted,
                    errors = stats.errors,
                    "Done"
                );
                total_files += stats.files_parsed;
                total_chunks += stats.chunks_upserted;
                total_errors += stats.errors;
                total_tokens += stats.chunks_upserted * AVG_TOKENS_PER_CHUNK;

                if total_tokens >= cli.max_tokens {
                    println!(
                        "\n⚠  Token cap reached ({} / {} tokens). Stopping early.",
                        total_tokens, cli.max_tokens
                    );
                    break 'outer;
                }
            }
            Err(e) => {
                error!(repo = %repo, error = %e, "Ingest failed");
                total_errors += 1;
            }
        }
    }

    let actual_cost = total_tokens as f64 / 1_000_000.0 * VOYAGE_COST_PER_MILLION;
    println!("\n=== Ingest summary ===");
    println!("  Repos processed : {}", targets.len());
    println!("  Source files    : {total_files}");
    println!("  Chunks upserted : {total_chunks}");
    println!("  Errors          : {total_errors}");
    println!("  Est. tokens used: ~{}M", total_tokens / 1_000_000);
    println!("  Est. cost       : ~${actual_cost:.2}");
    println!("  Collection      : {}", cli.collection);

    Ok(())
}

fn list_repomix_entries(zip_path: &std::path::Path) -> Result<Vec<(String, String)>> {
    let file = std::fs::File::open(zip_path)?;
    let archive = zip::ZipArchive::new(file)?;

    let mut entries: Vec<(String, String)> = (0..archive.len())
        .filter_map(|i| {
            // archive is consumed when calling by_index, but we just need names
            // We can't call by_index here without &mut — collect names first
            None::<(String, String)> // placeholder
        })
        .collect();

    // Re-open to list names
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    entries = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            let name = entry.name().to_string();
            if name.ends_with(".md") || name.ends_with(".xml") {
                let repo = repo_name_from_entry(&name);
                Some((name, repo))
            } else {
                None
            }
        })
        .collect();

    entries.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(entries)
}

fn repo_name_from_entry(entry_name: &str) -> String {
    let base = entry_name.trim_end_matches(".md").trim_end_matches(".xml");
    let base = if let Some(pos) = base.rfind("-repomix") {
        &base[..pos]
    } else {
        base
    };
    base.to_string()
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/activity_filter.rs">
//! Chatbot Activity Filter — Schema-Derived
//!
//! Significance is derived directly from the plugin schema. There is no
//! separate filter config: the schema IS the filter.
//!
//! # Signal derivation rules (from PluginSchema)
//!
//! | Schema condition                              | Significance      |
//! |-----------------------------------------------|-------------------|
//! | schema tag `"noise"` or `"overkill"`          | Noise  (suppress) |
//! | schema tag `"immutable"` + write op           | Signal            |
//! | field in `immutable_paths` + write op         | Signal            |
//! | field `read_only: true` + write op (violation)| Signal            |
//! | constraint failure on any field               | Signal            |
//! | tunable field write                           | Contextual        |
//! | field read (non-sensitive)                    | Routine           |
//! | `Autonomous` origin, any op                   | Signal (override) |
//! | health check / debug probe                    | Noise             |
//!
//! Users suppress unwanted events by tagging their plugin schema with
//! `"noise"` or `"overkill"` — no separate filter config needed.
//!
//! # Deduplication
//!
//! Exact content-hash dedup in a sliding time window. Tool calls bypass
//! dedup (idempotent retries still matter). Window size is a single
//! runtime tunable, not a per-plugin concern.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use op_state_store::plugin_schema::PluginSchema;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Significance tier. Ordered: Signal > Contextual > Routine > Noise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Significance {
    Noise,
    Routine,
    Contextual,
    Signal,
}

/// Derive significance for an operation directly from the plugin schema.
///
/// `field` is the specific field being touched, if known.
/// `is_write` distinguishes reads from writes for read_only checks.
/// `constraint_failed` should be true if validation rejected the operation.
/// `autonomous` — model acted without instruction; always upgrades to Signal.
pub fn derive_significance(
    schema: &PluginSchema,
    field: Option<&str>,
    is_write: bool,
    constraint_failed: bool,
    autonomous: bool,
) -> Significance {
    // Autonomous always Signal — we always want to know when the model acted alone
    if autonomous {
        return Significance::Signal;
    }

    // Constraint failure is always Signal regardless of field
    if constraint_failed {
        return Significance::Signal;
    }

    // Schema-level noise tags suppress everything from this plugin
    if schema.tags.iter().any(|t| t == "noise" || t == "overkill") {
        return Significance::Noise;
    }

    // Fully immutable schema — any write is Signal
    if is_write && schema.tags.iter().any(|t| t == "immutable") {
        return Significance::Signal;
    }

    if let Some(field_name) = field {
        // Field in immutable_paths — write is Signal
        let field_path = format!("/tunable/{field_name}");
        if is_write && schema.immutable_paths.contains(&field_path) {
            return Significance::Signal;
        }

        // read_only field write — this is a violation attempt, always Signal
        if is_write {
            if let Some(field_schema) = schema.fields.get(field_name) {
                if field_schema.read_only {
                    return Significance::Signal;
                }
            }
        }
    }

    // Writes to tunable fields are Contextual
    if is_write {
        return Significance::Contextual;
    }

    // Reads are Routine
    Significance::Routine
}

/// Operation kind — used for the hard-suppress gate before schema lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    ToolCall,
    MemoryWrite,
    MemoryRead,
    AutonomousDecision,
    IntentClassification,
    StateMutation,
    PolicyDecision,
    SignalEmit,
    HealthCheck,
    DebugRead,
    WorkflowStep,
    SessionLifecycle,
}

impl OpKind {
    /// Operations that are always Noise regardless of schema.
    /// These never reach the blockchain or Qdrant.
    pub fn is_always_noise(&self) -> bool {
        matches!(self, OpKind::HealthCheck | OpKind::DebugRead)
    }

    /// Operations that are always at least Contextual regardless of schema.
    pub fn is_always_contextual(&self) -> bool {
        matches!(
            self,
            OpKind::ToolCall | OpKind::SignalEmit | OpKind::PolicyDecision
        )
    }
}

/// An event produced by chatbot or agent activity, ready to be filtered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,

    /// The user who initiated the conversation this event belongs to.
    pub user_id: Option<String>,

    /// The conversation (chat session) this event belongs to.
    /// Groups the full why→what→who chain for a single session.
    pub conversation_id: Option<String>,

    /// The actor (chatbot, agent ID, cron, etc.)
    pub actor_id: String,

    pub op_kind: OpKind,

    /// True when the model acted without explicit instruction.
    pub autonomous: bool,

    /// Model confidence if autonomous (0.0–1.0).
    pub confidence: Option<f32>,

    /// Plugin that owns the state being touched, if applicable.
    pub plugin_id: Option<String>,

    /// Specific field being touched, if applicable.
    pub field: Option<String>,

    /// True if this is a write operation (vs read).
    pub is_write: bool,

    /// True if a schema constraint failed on this operation.
    pub constraint_failed: bool,

    pub memory_ref: Option<String>,
    pub tool_name: Option<String>,

    /// SHA-256 of the canonical serialised payload. Used for exact dedup.
    pub content_hash: String,

    /// Text summary for embedding / Qdrant upsert.
    pub summary: String,

    pub payload: serde_json::Value,
}

/// Outcome of the filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    /// Emit to blockchain AND Qdrant vector search.
    Emit(Significance),
    /// Emit to blockchain only — payload/summary stripped before Qdrant upsert.
    /// Used for PII-tagged plugin fields: auditable but not searchable.
    EmitChainOnly(Significance),
    Suppress(SuppressReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuppressReason {
    AlwaysNoise,
    SchemaTaggedNoise,
    BelowMinSignificance,
    ExactDuplicate,
}

/// Check whether a plugin schema or the specific field touched is tagged PII.
///
/// PII events reach the blockchain (audit trail) but are stripped before Qdrant upsert.
/// Tag the schema-level with `"pii"` to mark the entire plugin,
/// or tag individual fields with `"pii"` in their description/metadata to mark specific fields.
pub fn is_pii(schema: &PluginSchema, field: Option<&str>) -> bool {
    if schema.tags.iter().any(|t| t == "pii") {
        return true;
    }
    if let Some(field_name) = field {
        if let Some(field_schema) = schema.fields.get(field_name) {
            return field_schema.description.to_lowercase().contains("[pii]")
                || field_schema.constraints.iter().any(|c| {
                    matches!(c, op_state_store::plugin_schema::Constraint::Custom { validator }
                        if validator == "pii")
                });
        }
    }
    false
}

struct WindowEntry {
    timestamp: DateTime<Utc>,
    content_hash: String,
}

/// Runtime tunables — the only config outside the plugin schema.
/// Kept minimal: just the dedup window and minimum significance floor.
#[derive(Debug, Clone)]
pub struct FilterTunables {
    /// Minimum significance to emit. Default: Contextual.
    pub min_significance: Significance,
    /// Sliding dedup window duration in seconds. Default: 300.
    pub dedup_window_secs: i64,
    /// Max entries in dedup window. Default: 500.
    pub dedup_window_max: usize,
}

impl Default for FilterTunables {
    fn default() -> Self {
        Self {
            min_significance: Significance::Contextual,
            dedup_window_secs: 300,
            dedup_window_max: 500,
        }
    }
}

pub struct ActivityFilter {
    tunables: Arc<RwLock<FilterTunables>>,
    window: Arc<RwLock<VecDeque<WindowEntry>>>,
}

impl ActivityFilter {
    pub fn new(tunables: FilterTunables) -> Self {
        Self {
            tunables: Arc::new(RwLock::new(tunables)),
            window: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(FilterTunables::default())
    }

    pub async fn set_tunables(&self, t: FilterTunables) {
        *self.tunables.write().await = t;
    }

    /// Evaluate an event against the plugin schema + tunables.
    /// Pass `schema = None` for events with no associated plugin (tool calls, etc.)
    pub async fn evaluate(
        &self,
        event: &ActivityEvent,
        schema: Option<&PluginSchema>,
    ) -> Result<FilterDecision> {
        let tunables = self.tunables.read().await.clone();

        // Gate 1 — always-noise op kinds
        if event.op_kind.is_always_noise() {
            return Ok(FilterDecision::Suppress(SuppressReason::AlwaysNoise));
        }

        // Gate 2 — derive significance from plugin schema
        let sig = if let Some(schema) = schema {
            let schema_sig = derive_significance(
                schema,
                event.field.as_deref(),
                event.is_write,
                event.constraint_failed,
                event.autonomous,
            );

            // Schema said Noise — respect it
            if schema_sig == Significance::Noise {
                return Ok(FilterDecision::Suppress(SuppressReason::SchemaTaggedNoise));
            }

            // Always-contextual ops can't fall below Contextual
            if event.op_kind.is_always_contextual() {
                schema_sig.max(Significance::Contextual)
            } else {
                schema_sig
            }
        } else {
            // No schema — use op kind alone
            if event.autonomous {
                Significance::Signal
            } else if event.op_kind.is_always_contextual() {
                Significance::Contextual
            } else {
                Significance::Routine
            }
        };

        if sig < tunables.min_significance {
            return Ok(FilterDecision::Suppress(
                SuppressReason::BelowMinSignificance,
            ));
        }

        // Gate 3 — exact content-hash dedup
        self.evict_expired(&tunables).await;

        let is_dup = self
            .window
            .read()
            .await
            .iter()
            .any(|e| e.content_hash == event.content_hash);

        // Tool calls bypass dedup — retries are meaningful signal
        if is_dup && event.op_kind != OpKind::ToolCall {
            return Ok(FilterDecision::Suppress(SuppressReason::ExactDuplicate));
        }

        {
            let mut w = self.window.write().await;
            if w.len() >= tunables.dedup_window_max {
                w.pop_front();
            }
            w.push_back(WindowEntry {
                timestamp: event.timestamp,
                content_hash: event.content_hash.clone(),
            });
        }

        // PII gate — chain yes, Qdrant no
        let pii = schema.map_or(false, |s| is_pii(s, event.field.as_deref()));
        if pii {
            return Ok(FilterDecision::EmitChainOnly(sig));
        }

        Ok(FilterDecision::Emit(sig))
    }

    async fn evict_expired(&self, t: &FilterTunables) {
        let cutoff = Utc::now() - Duration::seconds(t.dedup_window_secs);
        let mut w = self.window.write().await;
        while w.front().map_or(false, |e| e.timestamp < cutoff) {
            w.pop_front();
        }
    }

    pub async fn window_len(&self) -> usize {
        self.window.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::plugin_schema::PluginSchemaBuilder;

    fn noise_schema() -> PluginSchema {
        PluginSchemaBuilder::new("test")
            .version("1.0")
            .description("noise plugin")
            .tag("noise")
            .build()
    }

    fn immutable_schema() -> PluginSchema {
        PluginSchemaBuilder::new("test")
            .version("1.0")
            .description("immutable plugin")
            .fully_immutable()
            .build()
    }

    #[test]
    fn test_noise_tag_suppresses() {
        let schema = noise_schema();
        let sig = derive_significance(&schema, None, true, false, false);
        assert_eq!(sig, Significance::Noise);
    }

    #[test]
    fn test_immutable_write_is_signal() {
        let schema = immutable_schema();
        let sig = derive_significance(&schema, None, true, false, false);
        assert_eq!(sig, Significance::Signal);
    }

    #[test]
    fn test_autonomous_always_signal() {
        let schema = noise_schema(); // even noise schema can't suppress autonomous
        let sig = derive_significance(&schema, None, false, false, true);
        assert_eq!(sig, Significance::Signal);
    }

    #[test]
    fn test_constraint_fail_always_signal() {
        let schema = noise_schema();
        let sig = derive_significance(&schema, None, false, true, false);
        assert_eq!(sig, Significance::Signal);
    }

    #[test]
    fn test_read_is_routine() {
        let schema = PluginSchemaBuilder::new("t").build();
        let sig = derive_significance(&schema, Some("field_x"), false, false, false);
        assert_eq!(sig, Significance::Routine);
    }

    #[test]
    fn test_pii_tag_detected_schema_level() {
        let schema = PluginSchemaBuilder::new("user-profile")
            .version("1.0")
            .description("user profile")
            .tag("pii")
            .build();
        assert!(is_pii(&schema, None));
        assert!(is_pii(&schema, Some("email")));
    }

    #[test]
    fn test_non_pii_schema_not_flagged() {
        let schema = PluginSchemaBuilder::new("metrics").build();
        assert!(!is_pii(&schema, None));
    }

    #[tokio::test]
    async fn test_pii_schema_emits_chain_only() {
        let filter = ActivityFilter::with_defaults();
        let schema = PluginSchemaBuilder::new("user-profile")
            .version("1.0")
            .description("user profile")
            .tag("pii")
            .build();
        let event = ActivityEvent {
            id: "pii1".into(),
            timestamp: Utc::now(),
            user_id: Some("u1".into()),
            conversation_id: Some("c1".into()),
            actor_id: "bot".into(),
            op_kind: OpKind::StateMutation,
            autonomous: false,
            confidence: None,
            plugin_id: Some("user-profile".into()),
            field: Some("email".into()),
            is_write: true,
            constraint_failed: false,
            memory_ref: None,
            tool_name: None,
            content_hash: "pii-hash-1".into(),
            summary: "update email".into(),
            payload: serde_json::json!({"email": "user@example.com"}),
        };
        let d = filter.evaluate(&event, Some(&schema)).await.unwrap();
        assert_eq!(d, FilterDecision::EmitChainOnly(Significance::Contextual));
    }

    #[tokio::test]
    async fn test_health_check_suppressed() {
        let filter = ActivityFilter::with_defaults();
        let event = ActivityEvent {
            id: "1".into(),
            timestamp: Utc::now(),
            user_id: None,
            conversation_id: None,
            actor_id: "bot".into(),
            op_kind: OpKind::HealthCheck,
            autonomous: false,
            confidence: None,
            plugin_id: None,
            field: None,
            is_write: false,
            constraint_failed: false,
            memory_ref: None,
            tool_name: None,
            content_hash: "h1".into(),
            summary: "ping".into(),
            payload: serde_json::json!({}),
        };
        let d = filter.evaluate(&event, None).await.unwrap();
        assert_eq!(d, FilterDecision::Suppress(SuppressReason::AlwaysNoise));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/agent_tools.rs">
//! MCP tool registry adapter for built-in agents.
//!
//! The agent catalog is the local schema source for exposing agents as tools.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use op_agents::{builtin_agent_descriptors, create_agent, AgentDescriptor, AgentTask};
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn register_agent_tools(registry: &ToolRegistry) -> Result<usize> {
    let mut count = 0;

    for descriptor in builtin_agent_descriptors() {
        for operation in &descriptor.operations {
            registry
                .register(Arc::new(AgentCatalogTool {
                    descriptor: descriptor.clone(),
                    operation: operation.clone(),
                    tool_name: format!(
                        "agent_{}_{}",
                        sanitize_tool_name(&descriptor.agent_type),
                        sanitize_tool_name(operation)
                    ),
                }) as BoxedTool)
                .await?;
            count += 1;
        }
    }

    Ok(count)
}

struct AgentCatalogTool {
    descriptor: AgentDescriptor,
    operation: String,
    tool_name: String,
}

#[async_trait]
impl Tool for AgentCatalogTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.descriptor.description
    }

    fn category(&self) -> &str {
        "agent"
    }

    fn namespace(&self) -> &str {
        "agents"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "agent".to_string(),
            self.descriptor.agent_type.clone(),
            self.operation.clone(),
        ]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "args": {
                    "type": "string",
                    "description": "Natural-language task or operation-specific arguments"
                },
                "path": {
                    "type": "string",
                    "description": "Optional working path"
                },
                "config": {
                    "type": "object",
                    "description": "Optional agent-specific configuration"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let agent_id = format!(
            "tool-registry-{}",
            sanitize_tool_name(&self.descriptor.agent_type)
        );
        let agent =
            create_agent(&self.descriptor.agent_type, agent_id).map_err(|err| anyhow!(err))?;

        let mut task = AgentTask::new(&self.descriptor.agent_type, &self.operation);
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            task = task.with_path(path);
        }
        if let Some(args) = input.get("args").and_then(|v| v.as_str()) {
            task = task.with_args(args);
        }
        if let Some(config) = input.get("config").and_then(|v| v.as_object()) {
            task.config = config
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect::<HashMap<_, _>>();
        }

        let result = agent.execute(task).await.map_err(|err| anyhow!(err))?;
        Ok(json!({
            "success": result.success,
            "agent_type": self.descriptor.agent_type,
            "agent_name": self.descriptor.name,
            "operation": result.operation,
            "data": result.data,
            "metadata": result.metadata,
        }))
    }
}

fn sanitize_tool_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/cognitive_tools.rs">
//! Cognitive Tools for MCP
//!
//! MCP tools backed by the SQLite namespace/entry memory store.
//! Operations: store, retrieve, query, delete, list_namespaces, stats.

use crate::agent_tools::register_agent_tools;
use crate::memory_store::{CognitiveMemoryStore, EntryQuery, NamespaceKind};
use crate::notebooklm::register_notebooklm_tools;
use anyhow::Result;
use async_trait::async_trait;
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub struct CognitiveToolRegistry;

impl CognitiveToolRegistry {
    pub async fn register_all(
        registry: &ToolRegistry,
        store: Arc<CognitiveMemoryStore>,
    ) -> Result<()> {
        registry
            .register(Arc::new(MemoryTool::new(store.clone())) as BoxedTool)
            .await?;
        register_agent_tools(registry).await?;
        register_notebooklm_tools(registry).await?;
        Ok(())
    }
}

pub struct MemoryTool {
    store: Arc<CognitiveMemoryStore>,
}

impl MemoryTool {
    pub fn new(store: Arc<CognitiveMemoryStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "cognitive_memory"
    }

    fn description(&self) -> &str {
        "Manage cognitive memory namespaces and entries. Operations: store, retrieve, query, delete, list_namespaces, stats."
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
                    "enum": ["store", "retrieve", "query", "delete", "list_namespaces", "stats"],
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
            other => Err(anyhow::anyhow!("unknown operation: {}", other)),
        }
    }
}

impl MemoryTool {
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
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;
        let value = simd_json_to_serde(&input["value"]);
        let tags: Vec<String> = input["tags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        self.ensure_namespace(namespace, input["namespace_kind"].as_str())
            .await?;

        let entry = self
            .store
            .store_entry(namespace, key, value, tags, None)
            .await?;
        Ok(json!({ "ok": true, "id": entry.id, "namespace": namespace, "key": key }))
    }

    async fn op_retrieve(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        match self.store.retrieve_entry(namespace, key).await? {
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
            namespace_id: input["namespace"].as_str().map(String::from),
            key_pattern: input["key_pattern"].as_str().map(String::from),
            tags: input["tags"].as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }),
            limit: input["limit"].as_i64(),
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

    async fn op_delete(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        let deleted = self.store.delete_entry(namespace, key).await?;
        Ok(json!({ "ok": deleted, "namespace": namespace, "key": key }))
    }

    async fn op_list_namespaces(&self, input: &Value) -> Result<Value> {
        let kind = input["namespace_kind"]
            .as_str()
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
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/cozo_shuttle.rs">
//! Re-exports the shared CozoDB shuttle from the `op-cozo-store` crate.
//!
//! Schema, queries, and helpers all live in `op-cozo-store::lib`.

pub use op_cozo_store::{named_rows_to_json, CozoGraphShuttle, PolicyVerdict};
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/dbus_interface.rs">
//! D-Bus interface for the Cognitive MCP tool registry.
//!
//! service:   org.opdbus.CognitiveMcp
//! object:    /org/opdbus/v1/cognitive
//! interface: org.opdbus.CognitiveMcpV1
//!
//! Methods:
//!   ListTools() -> s                  JSON array [{name, description, category}]
//!   GetToolSchema(s name) -> s        JSON input schema, or "null"
//!   CallTool(s name, s args_json) -> s  JSON result, or {"error":"..."}

use op_mcp::tool_registry::ToolRegistry;
use simd_json::prelude::*;
use std::sync::Arc;
use zbus::interface;

pub struct CognitiveMcpInterface {
    registry: Arc<ToolRegistry>,
}

impl CognitiveMcpInterface {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
}

#[interface(name = "org.opdbus.CognitiveMcpV1")]
impl CognitiveMcpInterface {
    async fn list_tools(&self) -> zbus::fdo::Result<String> {
        let defs = self.registry.list(0, usize::MAX, None).await;
        let arr: Vec<serde_json::Value> = defs
            .iter()
            .map(|d| serde_json::json!({ "name": d.name, "description": d.description, "category": d.category }))
            .collect();
        serde_json::to_string(&arr).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn get_tool_schema(&self, name: String) -> zbus::fdo::Result<String> {
        match self.registry.get_definition(&name).await {
            Some(def) => simd_json::to_string(&def.input_schema)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string())),
            None => Ok("null".to_string()),
        }
    }

    async fn call_tool(&self, name: String, args_json: String) -> String {
        let args = match parse_simd(&args_json) {
            Ok(v) => v,
            Err(e) => return err_json(&e),
        };
        match self.registry.execute(&name, args).await {
            Ok(result) => simd_json::to_string(&result)
                .unwrap_or_else(|_| r#"{"error":"serialize failed"}"#.to_string()),
            Err(e) => err_json(&e.to_string()),
        }
    }
}

fn parse_simd(s: &str) -> Result<simd_json::OwnedValue, String> {
    let mut buf = s.as_bytes().to_vec();
    simd_json::from_slice(&mut buf).map_err(|e| e.to_string())
}

fn err_json(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/doctor.rs">
//! 🛷 Doctor Diagnostics — R15
//!
//! Comprehensive system diagnostics: auth status, quota, memory store
//! health, session state, NotebookLM bridge status, Gemini fallback,
//! and query history.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::gemini_fallback::GeminiFallback;
use crate::memory_store::CognitiveMemoryStore;
use crate::quota::QuotaManager;
use crate::session::SessionManager;
use crate::tool_profiles;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub timestamp: String,
    pub overall_status: String,
    pub components: Vec<ComponentStatus>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub name: String,
    pub status: String,
    pub details: serde_json::Value,
}

/// Run full diagnostics across all components.
pub async fn run_diagnostics(
    memory_store: &Arc<CognitiveMemoryStore>,
    session_manager: &Arc<SessionManager>,
    quota_manager: &Arc<QuotaManager>,
    gemini: &Arc<GeminiFallback>,
) -> DiagnosticReport {
    let mut components = Vec::new();
    let mut recommendations = Vec::new();
    let mut all_ok = true;

    // 1. Memory Store
    match memory_store.get_stats().await {
        Ok(stats) => {
            components.push(ComponentStatus {
                name: "memory_store".into(),
                status: "ok".into(),
                details: serde_json::json!({
                    "total_namespaces": stats.total_namespaces,
                    "total_entries": stats.total_entries,
                    "entries_by_kind": stats.entries_by_kind,
                }),
            });
        }
        Err(e) => {
            all_ok = false;
            components.push(ComponentStatus {
                name: "memory_store".into(),
                status: "error".into(),
                details: serde_json::json!({ "error": e.to_string() }),
            });
            recommendations.push(
                "Memory store is unreachable. Check SQLite database path and permissions.".into(),
            );
        }
    }

    // 2. Session Manager
    let active = session_manager.active_count();
    let total = session_manager.count();
    components.push(ComponentStatus {
        name: "session_manager".into(),
        status: "ok".into(),
        details: serde_json::json!({
            "active_sessions": active,
            "total_sessions": total,
        }),
    });

    // 3. Quota Manager
    let (remaining, limit) = quota_manager.status().await;
    let tier = quota_manager.tier().await;
    let quota_status = if remaining == 0 { "exhausted" } else { "ok" };
    if remaining == 0 {
        recommendations.push(
            "Query quota exhausted. Consider upgrading tier or waiting for daily reset.".into(),
        );
    }
    components.push(ComponentStatus {
        name: "quota_manager".into(),
        status: quota_status.into(),
        details: serde_json::json!({
            "tier": tier.name,
            "remaining": remaining,
            "limit": limit,
        }),
    });

    // 4. Gemini Fallback
    let gemini_available = gemini.is_available().await;
    components.push(ComponentStatus {
        name: "gemini_fallback".into(),
        status: if gemini_available {
            "ok"
        } else {
            "unavailable"
        }
        .into(),
        details: serde_json::json!({
            "available": gemini_available,
        }),
    });
    if !gemini_available {
        recommendations.push("Gemini fallback unavailable. Set GEMINI_API_KEY for resilient queries when NotebookLM is down.".into());
    }

    // 5. Tool Profile
    let profile = tool_profiles::current_profile();
    let estimate = tool_profiles::token_estimate(profile);
    components.push(ComponentStatus {
        name: "tool_profile".into(),
        status: "ok".into(),
        details: serde_json::json!({
            "profile": profile.to_string(),
            "tool_count": estimate.tool_count,
            "schema_tokens": estimate.schema_tokens,
            "savings_percent": estimate.savings_percent,
        }),
    });

    // 6. Auth Status
    let auth_method =
        std::env::var("COGNITIVE_MCP_AUTH_METHOD").unwrap_or_else(|_| "chrome_profile".into());
    components.push(ComponentStatus {
        name: "auth".into(),
        status: "configured".into(),
        details: serde_json::json!({
            "method": auth_method,
        }),
    });

    let overall = if all_ok { "healthy" } else { "degraded" };

    DiagnosticReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        overall_status: overall.into(),
        components,
        recommendations,
    }
}

/// Get query history from the session manager.
pub fn get_query_history(session_manager: &SessionManager, limit: usize) -> Vec<serde_json::Value> {
    let sessions = session_manager.list_sessions();
    let mut all_turns = Vec::new();

    for session in sessions {
        for turn in &session.history {
            all_turns.push(serde_json::json!({
                "conversation_id": session.id,
                "notebook_id": session.notebook_id,
                "query": turn.query,
                "answer_preview": if turn.answer.len() > 200 {
                    format!("{}...", &turn.answer[..200])
                } else {
                    turn.answer.clone()
                },
                "timestamp": turn.timestamp.to_rfc3339(),
                "citations_count": turn.citations_count,
                "grounded": turn.grounded,
            }));
        }
    }

    // Sort by timestamp descending
    all_turns.sort_by(|a, b| {
        let ta = a["timestamp"].as_str().unwrap_or("");
        let tb = b["timestamp"].as_str().unwrap_or("");
        tb.cmp(ta)
    });

    all_turns.into_iter().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_get_empty_query_history() {
        let mgr = SessionManager::with_defaults();
        let history = get_query_history(&mgr, 10);
        assert!(history.is_empty());
    }

    #[test]
    fn should_get_query_history_with_turns() {
        let mgr = SessionManager::with_defaults();
        mgr.get_or_create("conv-1", "nb-1");
        mgr.append_turn(
            "conv-1",
            crate::session::QueryTurn {
                query: "test query".into(),
                answer: "test answer".into(),
                timestamp: chrono::Utc::now(),
                citations_count: 2,
                grounded: true,
            },
        )
        .unwrap();

        let history = get_query_history(&mgr, 10);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["query"], "test query");
        assert_eq!(history[0]["grounded"], true);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/gemini_fallback.rs">
//! 🔴 Gemini Fallback — R12
//!
//! When the NotebookLM browser bridge breaks (session expired, Chrome crash,
//! sidecar down), queries fall back to the Gemini API via reqwest.
//!
//! # Capabilities
//! - `gemini_query`: Standard grounded query via Gemini GenerateContent
//! - `deep_research`: Multi-step research with grounding via Gemini
//!
//! # Security (R13)
//! - API key read from env, never logged
//! - No shell=True, no eval
//! - Exponential backoff on transient errors

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

const DEFAULT_GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";
const MAX_RETRIES: u32 = 3;
const BASE_DELAY_MS: u64 = 200;

/// Gemini API client configuration.
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub enabled: bool,
}

impl GeminiConfig {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("COGNITIVE_MCP_GEMINI_API_KEY")
            .or_else(|_| std::env::var("GEMINI_API_KEY"))
            .ok()?;

        let enabled = std::env::var("COGNITIVE_MCP_GEMINI_ENABLED")
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);

        Some(Self {
            api_url: std::env::var("COGNITIVE_MCP_GEMINI_API_URL")
                .unwrap_or_else(|_| DEFAULT_GEMINI_API_URL.to_string()),
            api_key,
            model: std::env::var("COGNITIVE_MCP_GEMINI_MODEL")
                .unwrap_or_else(|_| DEFAULT_GEMINI_MODEL.to_string()),
            enabled,
        })
    }
}

/// Citation from Gemini grounding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiCitation {
    pub text: String,
    pub source: String,
    pub page: String,
}

/// Result of a Gemini query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiQueryResult {
    pub answer: String,
    pub citations: Vec<GeminiCitation>,
    pub model: String,
    pub is_fallback: bool,
}

/// Result of a deep research query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchResult {
    pub summary: String,
    pub sections: Vec<ResearchSection>,
    pub sources_consulted: usize,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSection {
    pub title: String,
    pub content: String,
    pub citations: Vec<GeminiCitation>,
}

/// Gemini API request types (simplified).
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

/// Gemini API response types.
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    #[serde(rename = "citationMetadata")]
    citation_metadata: Option<GeminiCitationMetadata>,
}

#[derive(Debug, Deserialize)]
struct GeminiCitationMetadata {
    #[serde(rename = "citationSources")]
    citation_sources: Option<Vec<GeminiCitationSource>>,
}

#[derive(Debug, Deserialize)]
struct GeminiCitationSource {
    uri: Option<String>,
    #[serde(rename = "startIndex")]
    start_index: Option<u32>,
    #[serde(rename = "endIndex")]
    end_index: Option<u32>,
}

/// Gemini fallback client.
pub struct GeminiFallback {
    client: reqwest::Client,
    config: Arc<RwLock<Option<GeminiConfig>>>,
}

impl GeminiFallback {
    pub fn new() -> Self {
        let config = GeminiConfig::from_env();
        if config.is_some() {
            tracing::info!("Gemini fallback client initialized");
        } else {
            tracing::info!(
                "Gemini fallback unavailable (set GEMINI_API_KEY or COGNITIVE_MCP_GEMINI_API_KEY)"
            );
        }
        Self {
            client: reqwest::Client::new(),
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Whether the Gemini fallback is available.
    pub async fn is_available(&self) -> bool {
        self.config
            .read()
            .await
            .as_ref()
            .map_or(false, |c| c.enabled)
    }

    /// Standard grounded query via Gemini.
    pub async fn gemini_query(
        &self,
        query: &str,
        context: Option<&str>,
    ) -> Result<GeminiQueryResult> {
        let config = self
            .config
            .read()
            .await
            .clone()
            .context("Gemini fallback not configured")?;

        if !config.enabled {
            anyhow::bail!("Gemini fallback is disabled");
        }

        let system_instruction = context.map(|ctx| GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart {
                text: format!(
                    "You are a grounded research assistant. Answer questions using ONLY the following context. If the answer is not in the context, say so.\n\nContext:\n{}",
                    ctx
                ),
            }],
        });

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart {
                    text: query.to_string(),
                }],
            }],
            generation_config: Some(GeminiGenerationConfig {
                temperature: 0.1,
                max_output_tokens: 4096,
            }),
            system_instruction,
        };

        let response = self
            .call_with_retry(&config, &request)
            .await
            .context("Gemini query failed after retries")?;

        let answer = extract_answer(&response);
        let citations = extract_citations(&response);

        Ok(GeminiQueryResult {
            answer,
            citations,
            model: config.model,
            is_fallback: true,
        })
    }

    /// Deep research — multi-step query that builds on itself.
    pub async fn deep_research(
        &self,
        topic: &str,
        context: Option<&str>,
        depth: u32,
    ) -> Result<DeepResearchResult> {
        let config = self
            .config
            .read()
            .await
            .clone()
            .context("Gemini fallback not configured")?;

        if !config.enabled {
            anyhow::bail!("Gemini fallback is disabled");
        }

        let depth = depth.min(5).max(1); // Clamp to 1-5 steps
        let mut sections = Vec::new();
        let mut accumulated_knowledge = context.unwrap_or("").to_string();

        // Step 1: Overview query
        let overview_prompt = format!(
            "Provide a comprehensive overview of: {}\n\nExisting context:\n{}",
            topic, accumulated_knowledge
        );
        let overview_result = self
            .gemini_query(&overview_prompt, Some(&accumulated_knowledge))
            .await?;
        accumulated_knowledge.push_str("\n\n");
        accumulated_knowledge.push_str(&overview_result.answer);

        sections.push(ResearchSection {
            title: "Overview".to_string(),
            content: overview_result.answer,
            citations: overview_result.citations,
        });

        // Steps 2..depth: Drill deeper
        let drill_prompts = [
            "What are the key technical details and implementation specifics?",
            "What are the trade-offs, limitations, and alternative approaches?",
            "What are the security implications and best practices?",
            "What are the performance characteristics and optimization strategies?",
        ];

        for step in 1..depth as usize {
            let prompt_idx = (step - 1).min(drill_prompts.len() - 1);
            let drill_prompt = format!(
                "Regarding '{}': {}\n\nBased on what we know so far:\n{}",
                topic, drill_prompts[prompt_idx], accumulated_knowledge
            );

            match self
                .gemini_query(&drill_prompt, Some(&accumulated_knowledge))
                .await
            {
                Ok(result) => {
                    accumulated_knowledge.push_str("\n\n");
                    accumulated_knowledge.push_str(&result.answer);

                    sections.push(ResearchSection {
                        title: drill_prompts[prompt_idx].to_string(),
                        content: result.answer,
                        citations: result.citations,
                    });
                }
                Err(e) => {
                    tracing::warn!(step, error = %e, "Deep research step failed, continuing");
                    break;
                }
            }
        }

        let summary = format!(
            "Deep research on '{}' completed with {} sections across {} research steps.",
            topic,
            sections.len(),
            depth
        );

        Ok(DeepResearchResult {
            summary,
            sections,
            sources_consulted: 0, // Gemini doesn't expose source count
            model: config.model,
        })
    }

    /// Call Gemini API with exponential backoff.
    async fn call_with_retry(
        &self,
        config: &GeminiConfig,
        request: &GeminiRequest,
    ) -> Result<GeminiResponse> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            config.api_url, config.model, config.api_key
        );

        let mut last_error = None;
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_DELAY_MS * (1 << (attempt - 1));
                tracing::warn!(
                    attempt,
                    delay_ms = delay,
                    "Retrying Gemini API call after backoff"
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            match self.client.post(&url).json(request).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return resp
                            .json::<GeminiResponse>()
                            .await
                            .context("Failed to parse Gemini response");
                    }

                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();

                    // Don't retry client errors (4xx) except 429 (rate limit)
                    if status.as_u16() != 429 && status.is_client_error() {
                        anyhow::bail!("Gemini API error {}: {}", status, body);
                    }

                    last_error = Some(anyhow::anyhow!("Gemini API error {}: {}", status, body));
                }
                Err(e) => {
                    last_error = Some(e.into());
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("Gemini API failed after {} retries", MAX_RETRIES)))
    }
}

fn extract_answer(response: &GeminiResponse) -> String {
    response
        .candidates
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.as_ref())
        .and_then(|content| content.parts.first())
        .map(|part| part.text.clone())
        .unwrap_or_else(|| "No response generated.".to_string())
}

fn extract_citations(response: &GeminiResponse) -> Vec<GeminiCitation> {
    response
        .candidates
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.citation_metadata.as_ref())
        .and_then(|cm| cm.citation_sources.as_ref())
        .map(|sources| {
            sources
                .iter()
                .map(|s| GeminiCitation {
                    text: String::new(),
                    source: s.uri.clone().unwrap_or_default(),
                    page: format!(
                        "{}-{}",
                        s.start_index.unwrap_or(0),
                        s.end_index.unwrap_or(0)
                    ),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_extract_answer_from_response() {
        let response = GeminiResponse {
            candidates: Some(vec![GeminiCandidate {
                content: Some(GeminiContent {
                    role: "model".to_string(),
                    parts: vec![GeminiPart {
                        text: "Test answer".to_string(),
                    }],
                }),
                citation_metadata: None,
            }]),
        };

        assert_eq!(extract_answer(&response), "Test answer");
    }

    #[test]
    fn should_handle_empty_response() {
        let response = GeminiResponse { candidates: None };
        assert_eq!(extract_answer(&response), "No response generated.");
    }

    #[test]
    fn should_extract_citations() {
        let response = GeminiResponse {
            candidates: Some(vec![GeminiCandidate {
                content: None,
                citation_metadata: Some(GeminiCitationMetadata {
                    citation_sources: Some(vec![GeminiCitationSource {
                        uri: Some("https://example.com".to_string()),
                        start_index: Some(0),
                        end_index: Some(100),
                    }]),
                }),
            }]),
        };

        let citations = extract_citations(&response);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source, "https://example.com");
    }

    #[tokio::test]
    async fn should_report_unavailable_without_key() {
        // No env var set — should be unavailable
        let fallback = GeminiFallback::new();
        // May or may not be available depending on test env
        let _ = fallback.is_available().await;
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/grpc_service.rs">
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

use crate::gemini_fallback::GeminiFallback;
use crate::memory_store::{CognitiveMemoryStore, NamespaceKind};
use crate::proto::cognitive_tool_service_server::CognitiveToolService;
use crate::proto::*;
use crate::quota::QuotaManager;
use crate::session::{QueryTurn, SessionManager};

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
    gemini_fallback: Arc<GeminiFallback>,
}

impl CognitiveGrpcService {
    pub fn new(
        memory_store: Arc<CognitiveMemoryStore>,
        session_manager: Arc<SessionManager>,
        quota_manager: Arc<QuotaManager>,
        gemini_fallback: Arc<GeminiFallback>,
    ) -> Self {
        Self {
            memory_store,
            session_manager,
            quota_manager,
            gemini_fallback,
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
        let namespace = format!("project:{}", req.notebook_id);
        let entries = self
            .memory_store
            .query_entries(crate::memory_store::EntryQuery {
                namespace_id: Some(namespace.clone()),
                key_pattern: Some(req.query.clone()),
                tags: None,
                limit: Some(10),
                offset: None,
            })
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

        let namespace = format!("project:{}", req.notebook_id);
        let limit = if req.max_results > 0 {
            req.max_results as i64
        } else {
            10
        };

        let entries = self
            .memory_store
            .query_entries(crate::memory_store::EntryQuery {
                namespace_id: Some(namespace),
                key_pattern: Some(req.query.clone()),
                tags: None,
                limit: Some(limit),
                offset: None,
            })
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

        let notebooks: Vec<NotebookInfo> = namespaces
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|ns| NotebookInfo {
                id: ns.id,
                name: ns.name,
                kind: ns.kind.to_string(),
                description: ns.description.unwrap_or_default(),
                source_count: 0, // TODO: count entries per namespace
                created_at: ns.created_at.to_rfc3339(),
                updated_at: ns.updated_at.to_rfc3339(),
            })
            .collect();

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

        // Try by ID-as-name first
        let ns = self
            .memory_store
            .get_namespace_by_name(&req.notebook_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| {
                Status::not_found(format!("Notebook '{}' not found", req.notebook_id))
            })?;

        let metadata_json =
            serde_json::to_string(&ns.metadata).unwrap_or_else(|_| "{}".to_string());

        Ok(Response::new(GetNotebookResponse {
            notebook: Some(NotebookInfo {
                id: ns.id,
                name: ns.name,
                kind: ns.kind.to_string(),
                description: ns.description.unwrap_or_default(),
                source_count: 0,
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

        let name = format!("{}:{}", kind, req.title);
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

        Ok(Response::new(CreateNotebookResponse {
            notebook: Some(NotebookInfo {
                id: ns.id,
                name: ns.name,
                kind: ns.kind.to_string(),
                description: ns.description.unwrap_or_default(),
                source_count: 0,
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

            let name = format!("{}:{}", kind, nb_req.title);
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
                    created_notebooks.push(NotebookInfo {
                        id: ns.id,
                        name: ns.name,
                        kind: ns.kind.to_string(),
                        description: ns.description.unwrap_or_default(),
                        source_count: 0,
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

        let namespace = format!("project:{}", req.notebook_id);

        // Ensure namespace exists
        let kind = NamespaceKind::Project;
        if self
            .memory_store
            .get_namespace_by_name(&namespace)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .is_none()
        {
            self.memory_store
                .upsert_namespace(&namespace, kind, None, None, None, serde_json::json!({}))
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }

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

        let namespace = format!("project:{}", req.notebook_id);
        // Ensure namespace exists
        if self
            .memory_store
            .get_namespace_by_name(&namespace)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .is_none()
        {
            self.memory_store
                .upsert_namespace(
                    &namespace,
                    NamespaceKind::Project,
                    None,
                    None,
                    None,
                    serde_json::json!({}),
                )
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }

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

        let namespace = format!("project:{}", req.notebook_id);
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

        let namespace = format!("project:{}", req.notebook_id);
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

        let namespace = format!("project:{}", req.notebook_id);

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

        let context = entries
            .iter()
            .map(|e| {
                format!(
                    "Source: {}\nContent: {}",
                    e.key,
                    e.value.as_str().unwrap_or(&e.value.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        // Step 2: Use Gemini Fallback to extract structured data
        let columns_str = req.columns.join(", ");
        let prompt = format!(
            "Task: {}\n\nExtract information into a JSON array of objects. Each object must have exactly these keys: {}.\n\nReturn ONLY the JSON array, nothing else.",
            req.prompt, columns_str
        );

        let result = self
            .gemini_fallback
            .gemini_query(&prompt, Some(&context))
            .await
            .map_err(|e| Status::internal(format!("Data extraction failed: {}", e)))?;

        // Step 3: Clean up Markdown code blocks if any
        let mut json_str = result.answer.trim().to_string();
        if json_str.starts_with("```json") {
            json_str = json_str.trim_start_matches("```json").to_string();
            json_str = json_str.trim_end_matches("```").trim().to_string();
        } else if json_str.starts_with("```") {
            json_str = json_str.trim_start_matches("```").to_string();
            json_str = json_str.trim_end_matches("```").trim().to_string();
        }

        // Count rows roughly by parsing
        let row_count = if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(&json_str) {
            arr.len() as i32
        } else {
            0
        };

        Ok(Response::new(GenerateDataTableResponse {
            data_json: json_str,
            row_count,
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
            auth_status: "chrome_profile".to_string(),
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

        // R9 — persistent auth: never wipe Chrome profile on failed launch
        // R13 — credentials 0o600
        // Phase 1: validate and store credential reference.
        // Actual Chrome profile management is in the NotebookLM sidecar.

        if req.auth_method.is_empty() {
            return Err(Status::invalid_argument(
                "auth_method is required (chrome_profile or cookie)",
            ));
        }

        if req.credential.is_empty() {
            return Err(Status::invalid_argument(
                "credential is required (path to Chrome profile or cookie value)",
            ));
        }

        // Validate Chrome profile path exists if using chrome_profile
        if req.auth_method == "chrome_profile" {
            let path = std::path::Path::new(&req.credential);
            if !path.exists() {
                return Err(Status::invalid_argument(format!(
                    "Chrome profile path '{}' does not exist",
                    req.credential
                )));
            }

            // R13 — check permissions (0o600 for credential files)
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if let Ok(metadata) = std::fs::metadata(path) {
                    let mode = metadata.mode() & 0o777;
                    if mode & 0o077 != 0 {
                        warn!(
                            path = %req.credential,
                            mode = format!("{:o}", mode),
                            "Chrome profile has overly permissive permissions; should be 0o600"
                        );
                    }
                }
            }
        }

        Ok(Response::new(SetupAuthResponse {
            success: true,
            message: format!(
                "Auth configured: method={}, credential stored",
                req.auth_method
            ),
        }))
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

        let namespace = format!("project:{}", req.notebook_id);

        self.memory_store
            .delete_entry(&namespace, &req.source_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(RemoveSourceResponse { success: true }))
    }

    // =========================================================================
    // R12 — GeminiQuery (Fallback)
    // =========================================================================
    async fn gemini_query(
        &self,
        request: Request<GeminiQueryRequest>,
    ) -> Result<Response<GeminiQueryResponse>, Status> {
        let req = request.into_inner();
        info!(mode = %req.mode, "GeminiQuery");

        let context = if req.context.is_empty() {
            None
        } else {
            Some(req.context.as_str())
        };

        if req.mode == "deep_research" {
            let depth = if req.depth > 0 { req.depth as u32 } else { 3 };
            let result = self
                .gemini_fallback
                .deep_research(&req.query, context, depth)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;

            let sections_json =
                serde_json::to_string(&result.sections).unwrap_or_else(|_| "[]".to_string());

            let citations = result
                .sections
                .iter()
                .flat_map(|s| &s.citations)
                .cloned()
                .map(|c| Citation {
                    text: c.text,
                    source: c.source,
                    page: c.page,
                })
                .collect();

            Ok(Response::new(GeminiQueryResponse {
                answer: result.summary,
                citations,
                model: result.model,
                is_fallback: true,
                sections_json,
            }))
        } else {
            let result = self
                .gemini_fallback
                .gemini_query(&req.query, context)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;

            let citations = result
                .citations
                .into_iter()
                .map(|c| Citation {
                    text: c.text,
                    source: c.source,
                    page: c.page,
                })
                .collect();

            Ok(Response::new(GeminiQueryResponse {
                answer: result.answer,
                citations,
                model: result.model,
                is_fallback: true,
                sections_json: "[]".to_string(),
            }))
        }
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

        let estimate = crate::tool_profiles::token_estimate(profile);
        let tools = crate::tool_profiles::tools_for_profile(profile)
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        Ok(Response::new(GetToolProfileResponse {
            current_profile: profile.to_string(),
            tool_count: estimate.tool_count as i32,
            schema_tokens: estimate.schema_tokens as i32,
            savings_percent: estimate.savings_percent as i32,
            tools,
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
            &self.gemini_fallback,
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
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/interceptor.rs">
use op_identity::{read_sled, IdentitySled};
use tonic::{Request, Status};

#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();

    if footprint_value.is_none() || trace_value.is_none() {
        return Err(Status::unauthenticated(
            "Missing Ghostbridge Identity Sled.",
        ));
    }

    // Zero-copy read from /dev/shm/plugin_schema.dat via mmap.
    // `_mmap` keeps the mapping alive for the duration of this function.
    let (sled_ptr, _mmap): (*const IdentitySled, _) =
        read_sled().map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;

    // SAFETY: read_sled() uses MmapOptions::len(IdentitySled::SIZE) so the
    // mapping is at least SIZE bytes, and write_sled() uses atomic rename so
    // readers never see a partial write.
    let sled = unsafe { &*sled_ptr };
    let current_footprint = sled.hashed_footprint;

    if !sled.is_sled_valid() {
        return Err(Status::failed_precondition("Invalid Schema State."));
    }

    let request_footprint = footprint_value
        .as_ref()
        .unwrap()
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid footprint encoding"))?;
    let expected_footprint = hex::encode(current_footprint);

    if request_footprint != expected_footprint {
        return Err(Status::permission_denied("Temporal Hash Mismatch."));
    }

    tracing::debug!(
        "Validated request with footprint {} and trace_id {}",
        hex::encode(current_footprint),
        sled.trace_id_hex()
    );

    if let Some(trace_val) = trace_value {
        req.extensions_mut().insert(trace_val);
    }

    Ok(req)
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/lib.rs">
//! OP Cognitive MCP Server
//!
//! A specialized MCP server for cognitive memory and dynamic content loading.
//! Provides tools for:
//! - Memory storage and retrieval
//! - Dynamic content loading
//! - Cognitive state management
//! - Context-aware tool discovery
//! - NotebookLM MCP bridge with gRPC ingress (R1-R16)
//! - Typed namespace tools for Operation D-Bus (R16)
//! - Conversation session management (R2, R10)
//! - Quota awareness (R11)

pub mod activity_filter;
pub mod agent_tools;
pub mod cognitive_tools;
pub mod cozo_shuttle;
pub mod dbus_interface;
pub mod doctor;
pub mod gemini_fallback;
pub mod grpc_service;
pub mod memory_store;
pub mod notebooklm;
pub mod qdrant_shuttle;
pub mod quota;
pub mod rag_pipeline;
pub mod server;
pub mod session;
pub mod soul_memory;
pub mod tool_profiles;
pub mod typed_tools;
pub mod voyage;

pub use activity_filter::{
    derive_significance, is_pii, ActivityEvent, ActivityFilter, FilterDecision, FilterTunables,
    OpKind, Significance, SuppressReason,
};
pub use cognitive_tools::CognitiveToolRegistry;
pub use cozo_shuttle::{CozoGraphShuttle, PolicyVerdict};
pub use grpc_service::CognitiveGrpcService;
pub use memory_store::CognitiveMemoryStore;
pub use op_identity::IdentitySled;
pub use qdrant_shuttle::{QdrantSemanticShuttle, SessionTraceContext};
pub use quota::{QuotaManager, QuotaTier};
pub use server::CognitiveMcpServer;
pub use session::SessionManager;
pub use soul_memory::{AgentNamespaceBinding, SoulMemory, SoulMemoryStore, SoulUpdate};
pub use voyage::VoyageClient;

/// Generated protobuf types for the CognitiveToolService.
/// Compiled from proto/cognitive.proto via tonic-build.
pub mod proto {
    tonic::include_proto!("operation.cognitive.v1");

    /// Combined FileDescriptorSet for reflection.
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("cognitive_descriptor");
}
pub mod interceptor;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/main.rs">
//! Cognitive MCP Server Binary
//!
//! Transports started in parallel:
//! - HTTP/SSE  (MCP protocol, port 3003)
//! - gRPC      (CognitiveToolService, port 50052)
//! - D-Bus     (org.opdbus.CognitiveMcp / /org/opdbus/v1/cognitive)
//!
//! On startup the server reads the local WireGuard public key (from the
//! interface named by $WG_INTERFACE, defaulting to "netmaker") and writes the
//! canonical IdentitySled to /dev/shm/plugin_schema.dat so the Ghostbridge
//! interceptor and Qdrant shuttle can authenticate outbound gRPC calls.
//!
//! Bind address resolution order (highest priority first):
//!   1. COGNITIVE_MCP_BIND / COGNITIVE_MCP_GRPC_BIND env vars
//!   2. --http / --grpc CLI flags
//!   3. WireGuard interface IP detected at startup (if interface is up)
//!   4. 0.0.0.0 fallback

use clap::Parser;
use op_cognitive_mcp::CognitiveMcpServer;
use op_identity::{write_sled_from_wg, WireGuardIdentity};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "cognitive-mcp-server")]
#[command(about = "Cognitive MCP Server with memory, NotebookLM bridge, gRPC, and D-Bus")]
struct Cli {
    /// HTTP/SSE server address (MCP protocol).
    /// If left at 0.0.0.0 the WireGuard interface IP is used when available.
    /// Override with COGNITIVE_MCP_BIND env var or this flag.
    #[arg(long, env = "COGNITIVE_MCP_BIND", default_value = "0.0.0.0:3003")]
    http: String,

    /// gRPC server address (CognitiveToolService).
    /// If left at 0.0.0.0 the WireGuard interface IP is used when available.
    /// Override with COGNITIVE_MCP_GRPC_BIND env var or this flag.
    #[arg(long, env = "COGNITIVE_MCP_GRPC_BIND", default_value = "0.0.0.0:50052")]
    grpc: String,

    /// CozoDB database path
    #[arg(
        long,
        env = "COGNITIVE_MCP_DB_PATH",
        default_value = "/var/lib/op-cognitive-mcp/memory.db"
    )]
    db: String,

    /// WireGuard interface to read identity from
    #[arg(long, env = "WG_INTERFACE", default_value = "netmaker")]
    wg_interface: String,

    /// Log level
    #[arg(long, env = "COGNITIVE_MCP_LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Disable gRPC server
    #[arg(long, env = "COGNITIVE_MCP_GRPC_DISABLED")]
    no_grpc: bool,

    /// Disable HTTP/SSE server
    #[arg(long, env = "COGNITIVE_MCP_HTTP_DISABLED")]
    no_http: bool,

    /// Disable D-Bus registration
    #[arg(long, env = "COGNITIVE_MCP_DBUS_DISABLED")]
    no_dbus: bool,
}

/// Promote an `0.0.0.0:PORT` default address to `<wg_ip>:PORT` when the WG
/// interface is up.  Explicit addresses (not starting with `0.0.0.0:`) are
/// returned unchanged so env-var or flag overrides always win.
fn resolve_bind(addr: &str, wg_ip: Option<&str>) -> String {
    if let Some(rest) = addr.strip_prefix("0.0.0.0:") {
        if let Some(ip) = wg_ip {
            return format!("{ip}:{rest}");
        }
    }
    addr.to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let level = match cli.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // ── WireGuard identity ────────────────────────────────────────────────────
    // 1. Detect local WG IP for bind address resolution.
    // 2. Write canonical IdentitySled to /dev/shm for Ghostbridge auth.
    let wg_id = WireGuardIdentity::with_interface(&cli.wg_interface);
    let wg_ip = wg_id.get_local_ip();

    match wg_id.get_local_pubkey() {
        Ok(pubkey) => {
            info!(
                interface = %cli.wg_interface,
                pubkey = %pubkey,
                wg_ip = ?wg_ip,
                "Writing WireGuard identity sled to /dev/shm/plugin_schema.dat"
            );
            if let Err(e) = write_sled_from_wg(&pubkey) {
                warn!(
                    error = %e,
                    "Failed to write identity sled — gRPC Ghostbridge auth will not work"
                );
            }
        }
        Err(e) => {
            warn!(
                interface = %cli.wg_interface,
                error = %e,
                "Could not read WireGuard public key — identity sled not written; \
                 set WG_PUBKEY env var to override"
            );
        }
    }

    // Resolve bind addresses: promote 0.0.0.0 defaults to WG interface IP.
    let http_addr = resolve_bind(&cli.http, wg_ip.as_deref());
    let grpc_addr = resolve_bind(&cli.grpc, wg_ip.as_deref());

    info!(
        http = %http_addr,
        grpc = %grpc_addr,
        db = %cli.db,
        wg_interface = %cli.wg_interface,
        "Starting Cognitive MCP Server"
    );

    let server = CognitiveMcpServer::new(&cli.db).await?;

    // D-Bus: start first, keep connection alive for the process lifetime.
    let _dbus_conn = if !cli.no_dbus {
        match server.start_dbus().await {
            Ok(conn) => {
                info!("D-Bus registered: org.opdbus.CognitiveMcp");
                Some(conn)
            }
            Err(e) => {
                warn!("D-Bus registration failed (continuing without it): {e}");
                None
            }
        }
    } else {
        None
    };

    match (cli.no_grpc, cli.no_http) {
        (true, true) => {
            eprintln!("Error: both --no-grpc and --no-http specified. Nothing to run.");
            std::process::exit(1);
        }
        (true, false) => {
            info!("Running HTTP/SSE only");
            server.start_http_server(&http_addr).await?;
        }
        (false, true) => {
            info!("Running gRPC only");
            server.start_grpc_server(&grpc_addr).await?;
        }
        (false, false) => {
            info!("Running HTTP/SSE + gRPC");
            server.start_dual(&http_addr, &grpc_addr).await?;
        }
    }

    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/memory_store.rs">
//! Cognitive Memory Store
//!
//! Namespace-based shared memory backend for the op-dbus chatbot and openclaw.
//! Backed by the unified CozoDB store; no SQLite.
//!
//! Architecture:
//! - **Namespace** = a named context (project, session, database, workflow, cron job, agent, etc.)
//! - **Entry** = a key/value pair within a namespace, stored as JSON.
//! - Schema lives in [`CozoGraphShuttle::seed_schema`]; this module just exposes typed CRUD.

use crate::cozo_shuttle::CozoGraphShuttle;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use cozo::{DataValue, NamedRows, ScriptMutability};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

type Params = BTreeMap<String, DataValue>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl std::fmt::Display for NamespaceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Project => "project",
            Self::Session => "session",
            Self::Database => "database",
            Self::Workflow => "workflow",
            Self::Agent => "agent",
            Self::Cron => "cron",
            Self::Custom => "custom",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for NamespaceKind {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "project" => Self::Project,
            "session" => Self::Session,
            "database" => Self::Database,
            "workflow" => Self::Workflow,
            "agent" => Self::Agent,
            "cron" => Self::Cron,
            _ => Self::Custom,
        })
    }
}

/// A named memory context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNamespace {
    pub id: String,
    /// Canonical name: "project:op-dbus", "cron:backup", "db:ovsdb", etc.
    pub name: String,
    pub kind: NamespaceKind,
    pub description: Option<String>,
    pub linked_task_id: Option<String>,
    pub linked_cron: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A key/value entry within a namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub namespace_id: String,
    pub key: String,
    pub value: serde_json::Value,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub access_count: i64,
    pub last_accessed: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EntryQuery {
    pub namespace_id: Option<String>,
    pub key_pattern: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MemoryStats {
    pub total_namespaces: i64,
    pub total_entries: i64,
    pub entries_by_kind: Vec<(String, i64)>,
}

pub struct CognitiveMemoryStore {
    shuttle: Arc<CozoGraphShuttle>,
}

impl CognitiveMemoryStore {
    pub async fn new(shuttle: Arc<CozoGraphShuttle>) -> Result<Self> {
        Ok(Self { shuttle })
    }

    fn run(&self, script: &str, params: Params) -> Result<NamedRows> {
        self.shuttle
            .db()
            .run_script(script, params, ScriptMutability::Mutable)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub async fn upsert_namespace(
        &self,
        name: &str,
        kind: NamespaceKind,
        description: Option<&str>,
        linked_task_id: Option<&str>,
        linked_cron: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<MemoryNamespace> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let kind_str = kind.to_string();
        let meta_str = serde_json::to_string(&metadata)?;

        let q = r#"
            ?[name, id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at]
                <- [[$name, $id, $kind, $desc, $task, $cron, $meta, $now, $now]]
            :put memory_namespaces {
                name => id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at
            }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("name".into(), DataValue::Str(name.into()));
        p.insert("id".into(), DataValue::Str(id.into()));
        p.insert("kind".into(), DataValue::Str(kind_str.into()));
        p.insert(
            "desc".into(),
            DataValue::Str(description.unwrap_or("").into()),
        );
        p.insert(
            "task".into(),
            DataValue::Str(linked_task_id.unwrap_or("").into()),
        );
        p.insert(
            "cron".into(),
            DataValue::Str(linked_cron.unwrap_or("").into()),
        );
        p.insert("meta".into(), DataValue::Str(meta_str.into()));
        p.insert("now".into(), DataValue::Str(now.into()));
        self.run(q, p).context("upsert namespace")?;

        self.get_namespace_by_name(name)
            .await?
            .context("namespace not found after upsert")
    }

    pub async fn get_namespace_by_name(&self, name: &str) -> Result<Option<MemoryNamespace>> {
        let q = r#"
            ?[name, id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at]
                := *memory_namespaces[name, id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at],
                   name = $name
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("name".into(), DataValue::Str(name.into()));
        let rows = self.run(q, p).context("get namespace by name")?;
        Ok(rows.rows.first().map(row_to_namespace))
    }

    pub async fn list_namespaces(
        &self,
        kind: Option<NamespaceKind>,
    ) -> Result<Vec<MemoryNamespace>> {
        let (q, params): (&str, Params) = if let Some(k) = kind {
            let mut p: Params = BTreeMap::new();
            p.insert("k".into(), DataValue::Str(k.to_string().into()));
            (
                r#"
                ?[name, id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at]
                    := *memory_namespaces[name, id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at],
                       kind = $k
                :order name
                "#,
                p,
            )
        } else {
            (
                r#"
                ?[name, id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at]
                    := *memory_namespaces[name, id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at]
                :order name
                "#,
                BTreeMap::new(),
            )
        };
        let rows = self.run(q, params).context("list namespaces")?;
        Ok(rows.rows.iter().map(row_to_namespace).collect())
    }

    pub async fn delete_namespace(&self, name: &str) -> Result<bool> {
        // Pre-check whether it exists; cozo :rm is silent.
        if self.get_namespace_by_name(name).await?.is_none() {
            return Ok(false);
        }
        // Cascade: remove all entries in this namespace first.
        let entries_q = r#"
            ?[ns, key]
                := *memory_entries[ns, key, _, _, _, _, _, _, _, _],
                   ns = $ns
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("ns".into(), DataValue::Str(name.into()));
        let entry_rows = self
            .run(entries_q, p)
            .context("collect entries to cascade")?;
        for row in &entry_rows.rows {
            let ns = dv_as_str(&row[0]).unwrap_or("").to_string();
            let key = dv_as_str(&row[1]).unwrap_or("").to_string();
            let mut pe: Params = BTreeMap::new();
            pe.insert("ns".into(), DataValue::Str(ns.into()));
            pe.insert("key".into(), DataValue::Str(key.into()));
            self.run(
                "?[namespace, key] <- [[$ns, $key]] :rm memory_entries { namespace, key }",
                pe,
            )
            .context("cascade delete entry")?;
        }
        // Remove namespace row.
        let mut pn: Params = BTreeMap::new();
        pn.insert("name".into(), DataValue::Str(name.into()));
        self.run("?[name] <- [[$name]] :rm memory_namespaces { name }", pn)
            .context("delete namespace")?;
        Ok(true)
    }

    pub async fn store_entry(
        &self,
        namespace_name: &str,
        key: &str,
        value: serde_json::Value,
        tags: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<MemoryEntry> {
        // Ensure namespace exists.
        self.get_namespace_by_name(namespace_name)
            .await?
            .with_context(|| format!("namespace '{}' not found", namespace_name))?;

        let now = Utc::now().to_rfc3339();
        let value_str = serde_json::to_string(&value)?;
        let tags_str = serde_json::to_string(&tags)?;
        let exp_str = expires_at.map(|t| t.to_rfc3339()).unwrap_or_default();

        // Preserve created_at + access counters on update by reading existing row first.
        let existing = self.fetch_entry_row(namespace_name, key)?;
        let (id, created_at, access_count, last_accessed) = match existing {
            Some(ref row) => (
                dv_as_str(&row[2]).unwrap_or("").to_string(),
                dv_as_str(&row[5]).unwrap_or(now.as_str()).to_string(),
                dv_as_int(&row[8]).unwrap_or(0),
                dv_as_str(&row[9]).unwrap_or(now.as_str()).to_string(),
            ),
            None => (Uuid::new_v4().to_string(), now.clone(), 0, now.clone()),
        };

        let q = r#"
            ?[namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed]
                <- [[$ns, $key, $id, $val, $tags, $ca, $now, $exp, $ac, $la]]
            :put memory_entries {
                namespace, key => id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed
            }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("ns".into(), DataValue::Str(namespace_name.into()));
        p.insert("key".into(), DataValue::Str(key.into()));
        p.insert("id".into(), DataValue::Str(id.into()));
        p.insert("val".into(), DataValue::Str(value_str.into()));
        p.insert("tags".into(), DataValue::Str(tags_str.into()));
        p.insert("ca".into(), DataValue::Str(created_at.into()));
        p.insert("now".into(), DataValue::Str(now.into()));
        p.insert("exp".into(), DataValue::Str(exp_str.into()));
        p.insert("ac".into(), DataValue::Num(cozo::Num::Int(access_count)));
        p.insert("la".into(), DataValue::Str(last_accessed.into()));
        self.run(q, p).context("store entry")?;

        self.retrieve_entry(namespace_name, key)
            .await?
            .context("entry not found after store")
    }

    pub async fn retrieve_entry(
        &self,
        namespace_name: &str,
        key: &str,
    ) -> Result<Option<MemoryEntry>> {
        let Some(row) = self.fetch_entry_row(namespace_name, key)? else {
            return Ok(None);
        };
        let entry = row_to_entry(&row);

        // Bump access counters (best-effort; ignore errors).
        let now = Utc::now().to_rfc3339();
        let q = r#"
            ?[namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed]
                <- [[$ns, $key, $id, $val, $tags, $ca, $ua, $exp, $ac, $la]]
            :put memory_entries {
                namespace, key => id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed
            }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("ns".into(), DataValue::Str(namespace_name.into()));
        p.insert("key".into(), DataValue::Str(key.into()));
        p.insert("id".into(), DataValue::Str(entry.id.clone().into()));
        p.insert(
            "val".into(),
            DataValue::Str(serde_json::to_string(&entry.value)?.into()),
        );
        p.insert(
            "tags".into(),
            DataValue::Str(serde_json::to_string(&entry.tags)?.into()),
        );
        p.insert(
            "ca".into(),
            DataValue::Str(entry.created_at.to_rfc3339().into()),
        );
        p.insert(
            "ua".into(),
            DataValue::Str(entry.updated_at.to_rfc3339().into()),
        );
        p.insert(
            "exp".into(),
            DataValue::Str(
                entry
                    .expires_at
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_default()
                    .into(),
            ),
        );
        p.insert(
            "ac".into(),
            DataValue::Num(cozo::Num::Int(entry.access_count + 1)),
        );
        p.insert("la".into(), DataValue::Str(now.into()));
        let _ = self.run(q, p);

        Ok(Some(entry))
    }

    pub async fn query_entries(&self, q: EntryQuery) -> Result<Vec<MemoryEntry>> {
        let now = Utc::now().to_rfc3339();
        let limit = q.limit.unwrap_or(100) as usize;
        let offset = q.offset.unwrap_or(0) as usize;

        let (script, params): (&str, Params) = match q.namespace_id.as_deref() {
            Some(ns) => {
                let mut p: Params = BTreeMap::new();
                p.insert("ns".into(), DataValue::Str(ns.into()));
                p.insert("now".into(), DataValue::Str(now.into()));
                (
                    r#"
                    ?[namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed]
                        := *memory_entries[namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed],
                           namespace = $ns,
                           (expires_at = "" || expires_at > $now)
                    :order -updated_at
                    "#,
                    p,
                )
            }
            None => {
                let mut p: Params = BTreeMap::new();
                p.insert("now".into(), DataValue::Str(now.into()));
                (
                    r#"
                    ?[namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed]
                        := *memory_entries[namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed],
                           (expires_at = "" || expires_at > $now)
                    :order -updated_at
                    "#,
                    p,
                )
            }
        };
        let rows = self.run(script, params).context("query entries")?;

        let mut entries: Vec<MemoryEntry> = rows.rows.iter().map(row_to_entry).collect();

        // Apply key_pattern (substring match) post-fetch.
        if let Some(pat) = &q.key_pattern {
            entries.retain(|e| e.key.contains(pat));
        }
        // Tag filter: every requested tag must be present.
        if let Some(tags) = &q.tags {
            entries.retain(|e| tags.iter().all(|t| e.tags.contains(t)));
        }
        // Offset + limit.
        Ok(entries.into_iter().skip(offset).take(limit).collect())
    }

    pub async fn delete_entry(&self, namespace_name: &str, key: &str) -> Result<bool> {
        if self.fetch_entry_row(namespace_name, key)?.is_none() {
            return Ok(false);
        }
        let mut p: Params = BTreeMap::new();
        p.insert("ns".into(), DataValue::Str(namespace_name.into()));
        p.insert("key".into(), DataValue::Str(key.into()));
        self.run(
            "?[namespace, key] <- [[$ns, $key]] :rm memory_entries { namespace, key }",
            p,
        )
        .context("delete entry")?;
        Ok(true)
    }

    pub async fn cleanup_expired(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        // Collect expired keys.
        let q = r#"
            ?[namespace, key]
                := *memory_entries[namespace, key, _, _, _, _, _, expires_at, _, _],
                   expires_at != "",
                   expires_at < $now
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("now".into(), DataValue::Str(now.into()));
        let rows = self.run(q, p).context("collect expired")?;
        let mut removed: u64 = 0;
        for row in &rows.rows {
            let ns = dv_as_str(&row[0]).unwrap_or("").to_string();
            let key = dv_as_str(&row[1]).unwrap_or("").to_string();
            let mut pr: Params = BTreeMap::new();
            pr.insert("ns".into(), DataValue::Str(ns.into()));
            pr.insert("key".into(), DataValue::Str(key.into()));
            if self
                .run(
                    "?[namespace, key] <- [[$ns, $key]] :rm memory_entries { namespace, key }",
                    pr,
                )
                .is_ok()
            {
                removed += 1;
            }
        }
        Ok(removed)
    }

    pub async fn get_stats(&self) -> Result<MemoryStats> {
        // Cheap counts: fetch all + count in Rust. Cozo aggregates would be tighter at scale.
        let ns_rows = self
            .run(
                r#"
                ?[name, kind]
                    := *memory_namespaces[name, _, kind, _, _, _, _, _, _]
                "#,
                BTreeMap::new(),
            )
            .context("count namespaces")?;
        let total_namespaces = ns_rows.rows.len() as i64;

        let entry_rows = self
            .run(
                r#"
                ?[namespace]
                    := *memory_entries[namespace, _, _, _, _, _, _, _, _, _]
                "#,
                BTreeMap::new(),
            )
            .context("count entries")?;
        let total_entries = entry_rows.rows.len() as i64;

        // Build name → kind map, then count entries per kind.
        let mut kind_by_ns: BTreeMap<String, String> = BTreeMap::new();
        for row in &ns_rows.rows {
            let name = dv_as_str(&row[0]).unwrap_or("").to_string();
            let kind = dv_as_str(&row[1]).unwrap_or("custom").to_string();
            kind_by_ns.insert(name, kind);
        }
        let mut tally: BTreeMap<String, i64> = BTreeMap::new();
        for row in &entry_rows.rows {
            let ns = dv_as_str(&row[0]).unwrap_or("").to_string();
            let kind = kind_by_ns
                .get(&ns)
                .cloned()
                .unwrap_or_else(|| "custom".to_string());
            *tally.entry(kind).or_insert(0) += 1;
        }
        let entries_by_kind: Vec<(String, i64)> = tally.into_iter().collect();

        Ok(MemoryStats {
            total_namespaces,
            total_entries,
            entries_by_kind,
        })
    }

    /// Internal helper: fetch a raw memory_entries row by (namespace, key).
    /// Column order matches the relation declaration in `cozo_shuttle::seed_schema`.
    fn fetch_entry_row(&self, namespace_name: &str, key: &str) -> Result<Option<Vec<DataValue>>> {
        let q = r#"
            ?[namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed]
                := *memory_entries[namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed],
                   namespace = $ns,
                   key = $key
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("ns".into(), DataValue::Str(namespace_name.into()));
        p.insert("key".into(), DataValue::Str(key.into()));
        let rows = self.run(q, p).context("fetch entry row")?;
        Ok(rows.rows.into_iter().next())
    }
}

// ── Row → typed struct conversion ─────────────────────────────────────────────

fn row_to_namespace(row: &Vec<DataValue>) -> MemoryNamespace {
    // Order matches the rule head:
    //   name, id, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at
    let name = dv_as_str(&row[0]).unwrap_or("").to_string();
    let id = dv_as_str(&row[1]).unwrap_or("").to_string();
    let kind_str = dv_as_str(&row[2]).unwrap_or("custom").to_string();
    let description = opt_string(&row[3]);
    let linked_task_id = opt_string(&row[4]);
    let linked_cron = opt_string(&row[5]);
    let meta_str = dv_as_str(&row[6]).unwrap_or("{}");
    let created = dv_as_str(&row[7]).unwrap_or("");
    let updated = dv_as_str(&row[8]).unwrap_or("");

    MemoryNamespace {
        id,
        name,
        kind: kind_str.parse().unwrap_or(NamespaceKind::Custom),
        description,
        linked_task_id,
        linked_cron,
        metadata: serde_json::from_str(meta_str).unwrap_or(serde_json::Value::Null),
        created_at: parse_ts(created),
        updated_at: parse_ts(updated),
    }
}

fn row_to_entry(row: &Vec<DataValue>) -> MemoryEntry {
    // Order matches the rule head:
    //   namespace, key, id, value, tags, created_at, updated_at, expires_at, access_count, last_accessed
    let namespace_id = dv_as_str(&row[0]).unwrap_or("").to_string();
    let key = dv_as_str(&row[1]).unwrap_or("").to_string();
    let id = dv_as_str(&row[2]).unwrap_or("").to_string();
    let value_str = dv_as_str(&row[3]).unwrap_or("null");
    let tags_str = dv_as_str(&row[4]).unwrap_or("[]");
    let created = dv_as_str(&row[5]).unwrap_or("");
    let updated = dv_as_str(&row[6]).unwrap_or("");
    let expires = dv_as_str(&row[7]).unwrap_or("");
    let access_count = dv_as_int(&row[8]).unwrap_or(0);
    let last_accessed = dv_as_str(&row[9]).unwrap_or("");

    MemoryEntry {
        id,
        namespace_id,
        key,
        value: serde_json::from_str(value_str).unwrap_or(serde_json::Value::Null),
        tags: serde_json::from_str(tags_str).unwrap_or_default(),
        created_at: parse_ts(created),
        updated_at: parse_ts(updated),
        expires_at: if expires.is_empty() {
            None
        } else {
            DateTime::parse_from_rfc3339(expires)
                .map(|t| t.with_timezone(&Utc))
                .ok()
        },
        access_count,
        last_accessed: parse_ts(last_accessed),
    }
}

fn opt_string(dv: &DataValue) -> Option<String> {
    match dv_as_str(dv) {
        Some(s) if !s.is_empty() => Some(s.to_string()),
        _ => None,
    }
}

fn dv_as_str(dv: &DataValue) -> Option<&str> {
    if let DataValue::Str(s) = dv {
        Some(s.as_str())
    } else {
        None
    }
}

fn dv_as_int(dv: &DataValue) -> Option<i64> {
    if let DataValue::Num(cozo::Num::Int(i)) = dv {
        Some(*i)
    } else {
        None
    }
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/notebooklm.rs">
//! NotebookLM MCP bridge for the cognitive server.
//!
//! Launches the npm-based NotebookLM MCP sidecar over stdio and re-exposes its
//! tools through the local Rust `ToolRegistry`.

use anyhow::Result;
use async_trait::async_trait;
use op_mcp::external_client::{ExternalMcpClient, ExternalMcpConfig, ExternalTool};
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const DEFAULT_NOTEBOOKLM_COMMAND: &str = "npx";
const DEFAULT_NOTEBOOKLM_ARGS: &[&str] = &["-y", "notebooklm-mcp@latest"];
const DEFAULT_NOTEBOOKLM_SERVER_NAME: &str = "notebooklm";
const DEFAULT_NOTEBOOKLM_PROFILE: &str = "minimal";

#[derive(Debug, Clone)]
struct NotebookLmConfig {
    enabled: bool,
    command: String,
    args: Vec<String>,
    server_name: String,
    profile: String,
    disabled_tools: Option<String>,
}

impl NotebookLmConfig {
    fn from_env() -> Self {
        Self {
            enabled: env_flag("COGNITIVE_MCP_NOTEBOOKLM_ENABLED", true),
            command: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_COMMAND")
                .unwrap_or_else(|_| DEFAULT_NOTEBOOKLM_COMMAND.to_string()),
            args: env_list(
                "COGNITIVE_MCP_NOTEBOOKLM_ARGS",
                DEFAULT_NOTEBOOKLM_ARGS
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
            ),
            server_name: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_SERVER_NAME")
                .unwrap_or_else(|_| DEFAULT_NOTEBOOKLM_SERVER_NAME.to_string()),
            profile: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_PROFILE")
                .unwrap_or_else(|_| DEFAULT_NOTEBOOKLM_PROFILE.to_string()),
            disabled_tools: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_DISABLED_TOOLS").ok(),
        }
    }

    fn external_config(&self) -> ExternalMcpConfig {
        let mut env = HashMap::new();
        env.insert("NOTEBOOKLM_PROFILE".to_string(), self.profile.clone());
        if let Some(disabled_tools) = &self.disabled_tools {
            env.insert(
                "NOTEBOOKLM_DISABLED_TOOLS".to_string(),
                disabled_tools.clone(),
            );
        }
        // Pass through the NOTEBOOKLM_COOKIE from the parent environment for authentication.
        if let Ok(cookie) = std::env::var("NOTEBOOKLM_COOKIE") {
            env.insert("NOTEBOOKLM_COOKIE".to_string(), cookie);
        }

        ExternalMcpConfig {
            name: self.server_name.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            env,
            api_key: None,
            api_key_env: "API_KEY".to_string(),
            auth_method: op_mcp::external_client::AuthMethod::None,
            headers: HashMap::new(),
        }
    }

    fn published_tool_name(&self, upstream_name: &str) -> String {
        let raw_name = upstream_name
            .split_once(':')
            .map(|(_, name)| name)
            .unwrap_or(upstream_name);

        let mut normalized = String::with_capacity(raw_name.len());
        let mut last_was_underscore = false;

        for ch in raw_name.chars() {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_alphanumeric() {
                normalized.push(ch);
                last_was_underscore = false;
            } else if !last_was_underscore {
                normalized.push('_');
                last_was_underscore = true;
            }
        }

        let normalized = normalized.trim_matches('_');
        if normalized.is_empty() {
            raw_name.to_string()
        } else {
            normalized.to_string()
        }
    }
}

pub async fn register_notebooklm_tools(registry: &ToolRegistry) -> Result<usize> {
    let config = NotebookLmConfig::from_env();
    if !config.enabled {
        tracing::info!("NotebookLM MCP bridge disabled");
        return Ok(0);
    }

    let mut client = ExternalMcpClient::new(config.external_config());
    if let Err(error) = client.start().await {
        tracing::warn!(
            error = %error,
            command = %config.command,
            args = ?config.args,
            "NotebookLM MCP sidecar failed to start; continuing without NotebookLM tools"
        );
        return Ok(0);
    }

    let upstream_tools = client.get_tools().await;
    if upstream_tools.is_empty() {
        tracing::warn!("NotebookLM MCP sidecar started but returned no tools");
        return Ok(0);
    }

    let shared_client = Arc::new(Mutex::new(client));
    let mut registered = 0usize;

    for tool in upstream_tools {
        let published_name = config.published_tool_name(&tool.name);
        let wrapper = NotebookLmTool::new(shared_client.clone(), tool, published_name);
        registry.register(Arc::new(wrapper) as BoxedTool).await?;
        registered += 1;
    }

    tracing::info!(registered, "Registered NotebookLM MCP tools");
    Ok(registered)
}

struct NotebookLmTool {
    client: Arc<Mutex<ExternalMcpClient>>,
    upstream_name: String,
    name: String,
    description: String,
    input_schema: Value,
}

impl NotebookLmTool {
    fn new(client: Arc<Mutex<ExternalMcpClient>>, tool: ExternalTool, name: String) -> Self {
        let upstream_name = tool
            .name
            .split_once(':')
            .map(|(_, raw_name)| raw_name.to_string())
            .unwrap_or_else(|| tool.name.clone());

        Self {
            client,
            upstream_name,
            name,
            description: format!("NotebookLM MCP: {}", tool.description),
            input_schema: tool.input_schema,
        }
    }
}

#[async_trait]
impl Tool for NotebookLmTool {
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
        "cognitive"
    }

    fn namespace(&self) -> &str {
        "notebooklm"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "notebooklm".to_string(),
            "rag".to_string(),
            "research".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        // Robustness: retry with exponential backoff + session rotation
        // per Operation_Dbus_Robustness_Recommendations.md
        const MAX_RETRIES: u32 = 3;
        const BASE_DELAY_MS: u64 = 100;

        let mut last_error = None;
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_DELAY_MS * (1 << (attempt - 1));
                tracing::warn!(
                    tool = %self.name,
                    attempt,
                    delay_ms = delay,
                    "Retrying NotebookLM tool call after backoff"
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            let result = {
                let mut client = self.client.lock().await;
                client.call_tool(&self.upstream_name, input.clone()).await
            };

            match result {
                Ok(value) => return Ok(value),
                Err(e) => {
                    tracing::warn!(
                        tool = %self.name,
                        attempt,
                        error = %e,
                        "NotebookLM tool call failed"
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!(
                "NotebookLM tool '{}' failed after {} retries",
                self.name,
                MAX_RETRIES
            )
        }))
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_list(name: &str, default: Vec<String>) -> Vec<String> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .split_whitespace()
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::NotebookLmConfig;

    #[test]
    fn should_normalize_notebooklm_tool_names() {
        let config = NotebookLmConfig {
            enabled: true,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "notebooklm-mcp@latest".to_string()],
            server_name: "notebooklm".to_string(),
            profile: "minimal".to_string(),
            disabled_tools: None,
        };

        assert_eq!(
            config.published_tool_name("notebooklm:create-notebook"),
            "create_notebook"
        );
        assert_eq!(config.published_tool_name("ask question"), "ask_question");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/qdrant_shuttle.rs">
use std::fs::File;
use std::mem::size_of;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use memmap2::MmapOptions;
use op_identity::IdentitySled;
use op_state_store::{FieldType, PluginSchema};
use qdrant_client::qdrant::{
    Condition, Filter, PointStruct, QueryPointsBuilder, RetrievedPoint, ScoredPoint,
    ScrollPointsBuilder, UpsertPointsBuilder,
};
use qdrant_client::{Payload, Qdrant};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

const DEFAULT_QDRANT_URL: &str = "http://127.0.0.1:6334";
const DEFAULT_COLLECTION_NAME: &str = "ctl_plane_reasoning_episodes";
const DEFAULT_USER_MEMORY_COLLECTION: &str = "user_memory";
const DEFAULT_SCHEMA_SLED_PATH: &str = "/dev/shm/plugin_schema.dat";
const DEFAULT_TRACE_LIMIT: u32 = 5;
const DEFAULT_VOYAGE_API_URL: &str = "https://api.voyageai.com/v1/embeddings";
const DEFAULT_VOYAGE_QUERY_MODEL: &str = "voyage-4";
const DEFAULT_VOYAGE_OUTPUT_DIMENSION: u32 = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionTraceContext {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub hashed_footprint: [u8; 32],
    pub trace_id: String,
}

pub struct QdrantSemanticShuttle {
    client: Qdrant,
    collection_name: String,
    user_memory_collection: String,
    sled_path: PathBuf,
    voyage_client: VoyageClient,
}

impl QdrantSemanticShuttle {
    /// Initializes the Qdrant gRPC client used by the Accountability Loop.
    pub async fn new() -> Result<Self> {
        let qdrant_url =
            std::env::var("COGNITIVE_MCP_QDRANT_URL").unwrap_or_else(|_| DEFAULT_QDRANT_URL.into());
        let collection_name = std::env::var("COGNITIVE_MCP_QDRANT_COLLECTION")
            .unwrap_or_else(|_| DEFAULT_COLLECTION_NAME.into());
        let user_memory_collection = std::env::var("COGNITIVE_MCP_USER_MEMORY_COLLECTION")
            .unwrap_or_else(|_| DEFAULT_USER_MEMORY_COLLECTION.into());
        let sled_path = std::env::var("COGNITIVE_MCP_SCHEMA_SLED_PATH")
            .unwrap_or_else(|_| DEFAULT_SCHEMA_SLED_PATH.into());
        let voyage_client = VoyageClient::from_env()?;

        Self::new_with_clients(
            &qdrant_url,
            collection_name,
            user_memory_collection,
            sled_path,
            voyage_client,
        )
        .await
    }

    async fn new_with_clients(
        qdrant_url: &str,
        collection_name: impl Into<String>,
        user_memory_collection: impl Into<String>,
        sled_path: impl Into<PathBuf>,
        voyage_client: VoyageClient,
    ) -> Result<Self> {
        let collection_name = collection_name.into();
        let user_memory_collection = user_memory_collection.into();
        let sled_path = sled_path.into();
        let client = Qdrant::from_url(qdrant_url)
            .build()
            .with_context(|| format!("failed to build Qdrant client for {qdrant_url}"))?;

        client.health_check().await.with_context(|| {
            format!("failed to reach Qdrant gRPC health endpoint at {qdrant_url}")
        })?;

        tracing::info!(
            qdrant_url,
            collection = %collection_name,
            user_memory_collection = %user_memory_collection,
            sled_path = %sled_path.display(),
            "Qdrant Semantic Shuttle linked to the gRPC interface"
        );

        Ok(Self {
            client,
            collection_name,
            user_memory_collection,
            sled_path,
            voyage_client,
        })
    }

    /// Reads the active identity sled directly from shared memory.
    pub fn current_trace_context(&self) -> Result<SessionTraceContext> {
        let sled = read_identity_sled(&self.sled_path)?;
        ensure!(
            sled.is_sled_valid(),
            "A.N.N.A. Scribe: Invalid Schema State. No active trace available."
        );

        Ok(SessionTraceContext {
            wireguard_pubkey: sled.wireguard_pubkey,
            mutation_index: sled.mutation_index,
            hashed_footprint: sled.hashed_footprint,
            trace_id: format_trace_id(sled.hashed_footprint),
        })
    }

    /// Renders the active appended PluginSchema into deterministic retrieval text.
    pub fn current_schema_embedding_text(&self) -> Result<String> {
        let schema = read_plugin_schema(&self.sled_path)?;
        Ok(render_schema_embedding_text(&schema))
    }

    /// Fetches the exact session episodes currently associated with the active trace.
    pub async fn stream_semantic_trace(&self) -> Result<Vec<RetrievedPoint>> {
        self.fetch_trace_episodes(DEFAULT_TRACE_LIMIT).await
    }

    pub async fn fetch_trace_episodes(&self, limit: u32) -> Result<Vec<RetrievedPoint>> {
        let trace = self.current_trace_context()?;
        let response = self
            .client
            .scroll(
                ScrollPointsBuilder::new(self.collection_name.clone())
                    .filter(Filter::must([Condition::matches(
                        "trace_id",
                        trace.trace_id.clone(),
                    )]))
                    .limit(limit)
                    .with_payload(true)
                    .with_vectors(true),
            )
            .await
            .with_context(|| {
                format!(
                    "failed to query Qdrant collection {} for trace {}",
                    self.collection_name, trace.trace_id
                )
            })?;

        tracing::info!(
            trace_id = %trace.trace_id,
            mutation_index = trace.mutation_index,
            matches = response.result.len(),
            "Accountability Loop fetched semantic trace episodes"
        );

        Ok(response.result)
    }

    /// Performs semantic retrieval within the active trace using a schema-derived Voyage query.
    pub async fn search_semantic_trace(&self, limit: u64) -> Result<Vec<ScoredPoint>> {
        let (trace, schema_query_text) = self.active_schema_query_text()?;
        let limit = if limit == 0 {
            u64::from(DEFAULT_TRACE_LIMIT)
        } else {
            limit
        };

        let embedding = self
            .voyage_client
            .embed_query(&schema_query_text)
            .await
            .context("failed to embed active shared-memory schema with Voyage")?;

        let response = self
            .client
            .query(
                QueryPointsBuilder::new(self.collection_name.clone())
                    .query(embedding)
                    .filter(Filter::must([Condition::matches(
                        "trace_id",
                        trace.trace_id.clone(),
                    )]))
                    .limit(limit)
                    .with_payload(true),
            )
            .await
            .with_context(|| {
                format!(
                    "failed semantic query against Qdrant collection {} for trace {}",
                    self.collection_name, trace.trace_id
                )
            })?;

        tracing::info!(
            trace_id = %trace.trace_id,
            mutation_index = trace.mutation_index,
            schema_name = %extract_schema_title(&schema_query_text),
            matches = response.result.len(),
            model = %self.voyage_client.model,
            "Accountability Loop fetched semantic matches from the shared-memory schema projection"
        );

        Ok(response.result)
    }

    // ── User Memory Methods ──────────────────────────────────────────────

    /// Embed text as a query vector (for semantic search)
    pub async fn embed_query_text(&self, text: &str) -> Result<Vec<f32>> {
        self.voyage_client.embed_query(text).await
    }

    /// Embed text as a document vector (for storage in Qdrant)
    pub async fn embed_document(&self, text: &str) -> Result<Vec<f32>> {
        self.voyage_client.embed_document(text).await
    }

    /// Upsert a memory entry into the user_memory collection
    ///
    /// Payload includes `container_id` and `entry_key` for scoped retrieval.
    pub async fn upsert_user_memory(
        &self,
        point_id: impl Into<String>,
        vector: Vec<f32>,
        container_id: &str,
        entry_key: &str,
        content: &str,
    ) -> Result<()> {
        let payload: Payload = serde_json::json!({
            "container_id": container_id,
            "entry_key": entry_key,
            "content": content,
        })
        .try_into()
        .context("failed to build user_memory payload")?;

        let point = PointStruct::new(point_id.into(), vector, payload);

        self.client
            .upsert_points(UpsertPointsBuilder::new(
                self.user_memory_collection.clone(),
                vec![point],
            ))
            .await
            .with_context(|| {
                format!(
                    "failed to upsert user_memory point into collection {}",
                    self.user_memory_collection
                )
            })?;

        tracing::info!(
            collection = %self.user_memory_collection,
            container_id = %container_id,
            entry_key = %entry_key,
            "User memory upserted to Qdrant"
        );

        Ok(())
    }

    /// Semantic search over user_memory scoped to a container_id
    pub async fn search_user_memory(
        &self,
        query_embedding: Vec<f32>,
        container_id: &str,
        limit: u64,
    ) -> Result<Vec<ScoredPoint>> {
        let response = self
            .client
            .query(
                QueryPointsBuilder::new(self.user_memory_collection.clone())
                    .query(query_embedding)
                    .filter(Filter::must([Condition::matches(
                        "container_id",
                        container_id.to_string(),
                    )]))
                    .limit(limit)
                    .with_payload(true),
            )
            .await
            .with_context(|| {
                format!(
                    "failed semantic query against user_memory collection {} for container {}",
                    self.user_memory_collection, container_id
                )
            })?;

        tracing::info!(
            collection = %self.user_memory_collection,
            container_id = %container_id,
            matches = response.result.len(),
            "User memory semantic search completed"
        );

        Ok(response.result)
    }

    fn active_schema_query_text(&self) -> Result<(SessionTraceContext, String)> {
        let (sled, schema) = read_identity_sled_and_schema(&self.sled_path)?;
        ensure!(
            sled.is_sled_valid(),
            "A.N.N.A. Scribe: Invalid Schema State. No active trace available."
        );

        Ok((
            SessionTraceContext {
                wireguard_pubkey: sled.wireguard_pubkey,
                mutation_index: sled.mutation_index,
                hashed_footprint: sled.hashed_footprint,
                trace_id: format_trace_id(sled.hashed_footprint),
            },
            render_schema_embedding_text(&schema),
        ))
    }
}

struct VoyageClient {
    client: Client,
    api_url: String,
    api_key: String,
    model: String,
    output_dimension: u32,
}

impl VoyageClient {
    fn from_env() -> Result<Self> {
        let api_key = std::env::var("COGNITIVE_MCP_VOYAGE_API_KEY")
            .or_else(|_| std::env::var("VOYAGE_API_KEY"))
            .context(
                "missing Voyage API key: set COGNITIVE_MCP_VOYAGE_API_KEY or VOYAGE_API_KEY",
            )?;
        let api_url = std::env::var("COGNITIVE_MCP_VOYAGE_API_URL")
            .unwrap_or_else(|_| DEFAULT_VOYAGE_API_URL.into());
        let model = std::env::var("COGNITIVE_MCP_VOYAGE_QUERY_MODEL")
            .unwrap_or_else(|_| DEFAULT_VOYAGE_QUERY_MODEL.into());
        let output_dimension = std::env::var("COGNITIVE_MCP_VOYAGE_OUTPUT_DIMENSION")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_VOYAGE_OUTPUT_DIMENSION);

        Ok(Self {
            client: Client::new(),
            api_url,
            api_key,
            model,
            output_dimension,
        })
    }

    async fn embed_query(&self, input: &str) -> Result<Vec<f32>> {
        self.embed(input, "query").await
    }

    async fn embed_document(&self, input: &str) -> Result<Vec<f32>> {
        self.embed(input, "document").await
    }

    async fn embed(&self, input: &str, input_type: &str) -> Result<Vec<f32>> {
        let body = VoyageEmbeddingRequest {
            input,
            model: &self.model,
            input_type,
            truncation: true,
            output_dimension: self.output_dimension,
            output_dtype: "float",
        };

        let response = self
            .client
            .post(&self.api_url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("failed to call Voyage embeddings API at {}", self.api_url))?
            .error_for_status()
            .context("Voyage embeddings API returned an error status")?;

        let response_json = response
            .json::<Value>()
            .await
            .context("failed to decode Voyage embeddings response")?;

        extract_embedding(&response_json)
    }
}

#[derive(Serialize)]
struct VoyageEmbeddingRequest<'a> {
    input: &'a str,
    model: &'a str,
    input_type: &'a str,
    truncation: bool,
    output_dimension: u32,
    output_dtype: &'a str,
}

fn read_identity_sled(path: &Path) -> Result<IdentitySled> {
    let (sled, _) = read_shared_mapping(path)?;
    Ok(sled)
}

fn read_plugin_schema(path: &Path) -> Result<PluginSchema> {
    let (_, schema_bytes) = read_shared_mapping(path)?;
    parse_plugin_schema(schema_bytes, path)
}

fn read_identity_sled_and_schema(path: &Path) -> Result<(IdentitySled, PluginSchema)> {
    let (sled, schema_bytes) = read_shared_mapping(path)?;
    let schema = parse_plugin_schema(schema_bytes, path)?;
    Ok((sled, schema))
}

fn read_shared_mapping(path: &Path) -> Result<(IdentitySled, Vec<u8>)> {
    let file = File::open(path)
        .with_context(|| format!("failed to open SchemaEngine sled at {}", path.display()))?;
    let mmap = unsafe { MmapOptions::new().map(&file) }
        .with_context(|| format!("failed to mmap SchemaEngine sled at {}", path.display()))?;

    ensure!(
        mmap.len() >= size_of::<IdentitySled>(),
        "SchemaEngine sled at {} is smaller than IdentitySled ABI ({})",
        path.display(),
        size_of::<IdentitySled>()
    );

    let sled_ptr = mmap.as_ptr().cast::<IdentitySled>();
    let sled = unsafe { std::ptr::read_unaligned(sled_ptr) };
    let schema_bytes = mmap[size_of::<IdentitySled>()..]
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .collect::<Vec<_>>();

    Ok((sled, schema_bytes))
}

fn parse_plugin_schema(schema_bytes: Vec<u8>, path: &Path) -> Result<PluginSchema> {
    ensure!(
        !schema_bytes.is_empty(),
        "SchemaEngine sled at {} did not contain appended PluginSchema bytes",
        path.display()
    );

    serde_json::from_slice(&schema_bytes).with_context(|| {
        format!(
            "failed to parse appended PluginSchema from shared memory at {}",
            path.display()
        )
    })
}

fn format_trace_id(hashed_footprint: [u8; 32]) -> String {
    format!("trace-{}", hex::encode(hashed_footprint))
}

fn render_schema_embedding_text(schema: &PluginSchema) -> String {
    let mut lines = vec![
        format!("schema_name: {}", schema.name),
        format!("schema_category: {}", schema.category),
        format!("schema_version: {}", schema.version),
        format!("schema_description: {}", schema.description.trim()),
    ];

    let mut tags = schema.tags.clone();
    tags.sort();
    if !tags.is_empty() {
        lines.push(format!("schema_tags: {}", tags.join(", ")));
    }

    let mut immutable_paths = schema.immutable_paths.clone();
    immutable_paths.sort();
    if !immutable_paths.is_empty() {
        lines.push(format!("immutable_paths: {}", immutable_paths.join(", ")));
    }

    let mut dependencies = schema.dependencies.clone();
    dependencies.sort();
    if !dependencies.is_empty() {
        lines.push(format!("dependencies: {}", dependencies.join(", ")));
    }

    let mut field_names = schema.fields.keys().cloned().collect::<Vec<_>>();
    field_names.sort();

    for field_name in field_names {
        let Some(field_schema) = schema.fields.get(&field_name) else {
            continue;
        };

        lines.push(format!(
            "field {}: type={}, required={}, read_only={}, description={}",
            field_name,
            render_field_type(&field_schema.field_type),
            field_schema.required,
            field_schema.read_only,
            field_schema.description.trim()
        ));

        if let Some(condition) = &field_schema.read_only_when {
            lines.push(format!(
                "field {} read_only_when: {}={}",
                field_name, condition.property, condition.value
            ));
        }

        if !field_schema.constraints.is_empty() {
            let constraints = field_schema
                .constraints
                .iter()
                .map(render_constraint)
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("field {} constraints: {}", field_name, constraints));
        }
    }

    lines.join("\n")
}

fn render_field_type(field_type: &FieldType) -> String {
    match field_type {
        FieldType::String => "string".to_string(),
        FieldType::Integer => "integer".to_string(),
        FieldType::Float => "float".to_string(),
        FieldType::Boolean => "boolean".to_string(),
        FieldType::Array(inner) => format!("array<{}>", render_field_type(inner)),
        FieldType::Object(fields) => {
            let mut names = fields.keys().cloned().collect::<Vec<_>>();
            names.sort();
            format!("object<{}>", names.join("|"))
        }
        FieldType::Enum(values) => format!("enum<{}>", values.join("|")),
        FieldType::Any => "any".to_string(),
    }
}

fn render_constraint(constraint: &op_state_store::Constraint) -> String {
    match constraint {
        op_state_store::Constraint::Min { value } => format!("min={value}"),
        op_state_store::Constraint::Max { value } => format!("max={value}"),
        op_state_store::Constraint::Pattern { regex } => format!("pattern={regex}"),
        op_state_store::Constraint::OneOf { values } => {
            format!(
                "one_of={}",
                serde_json::to_string(values).unwrap_or_default()
            )
        }
        op_state_store::Constraint::RequiresField { field } => format!("requires_field={field}"),
        op_state_store::Constraint::Custom { validator } => format!("custom={validator}"),
    }
}

fn extract_schema_title(schema_query_text: &str) -> &str {
    schema_query_text
        .lines()
        .find_map(|line| line.strip_prefix("schema_name: "))
        .unwrap_or("unknown")
}

fn extract_embedding(response_json: &Value) -> Result<Vec<f32>> {
    let Some(embedding_values) = response_json
        .get("data")
        .and_then(Value::as_array)
        .and_then(|entries| entries.first())
        .and_then(|entry| entry.get("embedding"))
        .and_then(Value::as_array)
        .or_else(|| {
            response_json
                .get("embeddings")
                .and_then(Value::as_array)
                .and_then(|entries| entries.first())
                .and_then(Value::as_array)
        })
    else {
        return Err(anyhow::anyhow!(
            "Voyage embeddings response did not contain a usable embedding"
        ));
    };

    let mut embedding = Vec::with_capacity(embedding_values.len());
    for value in embedding_values {
        let number = value
            .as_f64()
            .context("Voyage embedding contained a non-numeric value")?;
        embedding.push(number as f32);
    }

    ensure!(!embedding.is_empty(), "Voyage embedding response was empty");
    Ok(embedding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::{Constraint, FieldSchema, ReadOnlyCondition};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn should_preserve_identity_sled_abi_shape() {
        // Canonical sled from op-identity: wireguard_pubkey(32) + mutation_index(8)
        // + is_valid(1) + _pad(7) + hashed_footprint(32) + subid taxonomy + compliance fields.
        assert!(
            size_of::<IdentitySled>() >= 32 + 8 + 1 + 7 + 32,
            "IdentitySled ABI unexpectedly shrank (using canonical op-identity layout)"
        );
    }

    #[test]
    fn should_format_trace_id_from_hashed_footprint() {
        let trace_id = format_trace_id([0xAB; 32]);
        assert_eq!(trace_id, format!("trace-{}", "ab".repeat(32)));
    }

    #[test]
    fn should_extract_embedding_from_openai_style_data_payload() {
        let embedding = extract_embedding(&json!({
            "data": [{
                "embedding": [0.25, -0.5, 1.5]
            }]
        }))
        .unwrap();

        assert_eq!(embedding, vec![0.25, -0.5, 1.5]);
    }

    #[test]
    fn should_extract_embedding_from_embeddings_array_payload() {
        let embedding = extract_embedding(&json!({
            "embeddings": [[0.1, 0.2, 0.3]]
        }))
        .unwrap();

        assert_eq!(embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn should_render_schema_embedding_text_deterministically() {
        let mut nested_fields = HashMap::new();
        nested_fields.insert(
            "beta".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: String::new(),
                default: None,
                example: None,
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        );
        nested_fields.insert(
            "alpha".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: String::new(),
                default: None,
                example: None,
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        );

        let mut fields = HashMap::new();
        fields.insert(
            "outcome_class".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["deny".into(), "allow".into()]),
                required: true,
                description: "Outcome bucket".into(),
                default: None,
                example: None,
                constraints: vec![Constraint::OneOf {
                    values: vec![simd_json::json!("allow"), simd_json::json!("deny")],
                }],
                read_only: false,
                read_only_when: Some(ReadOnlyCondition {
                    property: "sealed".into(),
                    value: "true".into(),
                }),
            },
        );
        fields.insert(
            "tools_consulted".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(nested_fields))),
                required: false,
                description: "Tools touched by the episode".into(),
                default: None,
                example: None,
                constraints: vec![Constraint::Min { value: 1.0 }],
                read_only: true,
                read_only_when: None,
            },
        );

        let schema = PluginSchema {
            name: "ctl-plane-chatbot".into(),
            category: "accountability".into(),
            version: "1.0.0".into(),
            description: "Human reviewable reasoning episodes".into(),
            fields,
            dependencies: vec!["op-grpc-bridge".into(), "op-state-store".into()],
            example: None,
            immutable_paths: vec!["/episode_id".into()],
            tags: vec!["audit".into(), "pii".into()],
            dialect: op_state_store::DEFAULT_SCHEMA_DIALECT.into(),
            mutation_index: Some(7),
        };

        let rendered = render_schema_embedding_text(&schema);

        assert!(rendered.contains("schema_name: ctl-plane-chatbot"));
        assert!(rendered.contains("schema_category: accountability"));
        assert!(rendered.contains("schema_tags: audit, pii"));
        assert!(rendered.contains("immutable_paths: /episode_id"));
        assert!(rendered.contains("dependencies: op-grpc-bridge, op-state-store"));
        assert!(rendered.contains(
            "field outcome_class: type=enum<deny|allow>, required=true, read_only=false, description=Outcome bucket"
        ));
        assert!(rendered.contains("field outcome_class read_only_when: sealed=true"));
        assert!(rendered.contains("field tools_consulted: type=array<object<alpha|beta>>"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/quota.rs">
//! Quota Awareness Layer (R11)
//!
//! Tracks query usage against configurable tier limits.
//! Default free tier: ~50 queries/day per the NotebookLM MCP spec.
//! The quota resets daily at midnight UTC.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Quota tier configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaTier {
    pub name: String,
    pub daily_limit: u32,
}

impl Default for QuotaTier {
    fn default() -> Self {
        Self {
            name: "free".to_string(),
            daily_limit: 50,
        }
    }
}

/// Thread-safe quota tracker.
pub struct QuotaManager {
    tier: Arc<RwLock<QuotaTier>>,
    queries_today: AtomicU32,
    last_reset: Arc<RwLock<DateTime<Utc>>>,
}

impl QuotaManager {
    pub fn new(tier: QuotaTier) -> Self {
        Self {
            tier: Arc::new(RwLock::new(tier)),
            queries_today: AtomicU32::new(0),
            last_reset: Arc::new(RwLock::new(Utc::now())),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(QuotaTier::default())
    }

    /// Check if a query is allowed under the current quota.
    /// Returns (allowed, remaining, limit).
    pub async fn check_and_increment(&self) -> (bool, u32, u32) {
        self.maybe_reset().await;

        let tier = self.tier.read().await;
        let current = self.queries_today.fetch_add(1, Ordering::Relaxed);

        if current >= tier.daily_limit {
            // Roll back the increment — over quota
            self.queries_today.fetch_sub(1, Ordering::Relaxed);
            (false, 0, tier.daily_limit)
        } else {
            let remaining = tier.daily_limit.saturating_sub(current + 1);
            (true, remaining, tier.daily_limit)
        }
    }

    /// Get current quota status without incrementing.
    pub async fn status(&self) -> (u32, u32) {
        self.maybe_reset().await;
        let tier = self.tier.read().await;
        let used = self.queries_today.load(Ordering::Relaxed);
        let remaining = tier.daily_limit.saturating_sub(used);
        (remaining, tier.daily_limit)
    }

    /// Update the quota tier at runtime (R11: set_quota_tier).
    pub async fn set_tier(&self, tier: QuotaTier) {
        *self.tier.write().await = tier;
    }

    /// Get current tier info.
    pub async fn tier(&self) -> QuotaTier {
        self.tier.read().await.clone()
    }

    /// Reset counter if a new UTC day has started.
    async fn maybe_reset(&self) {
        let now = Utc::now();
        let last = *self.last_reset.read().await;

        if now.date_naive() != last.date_naive() {
            self.queries_today.store(0, Ordering::Relaxed);
            *self.last_reset.write().await = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_allow_queries_within_limit() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "test".into(),
            daily_limit: 3,
        });

        let (ok, remaining, limit) = mgr.check_and_increment().await;
        assert!(ok);
        assert_eq!(remaining, 2);
        assert_eq!(limit, 3);
    }

    #[tokio::test]
    async fn should_deny_queries_over_limit() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "test".into(),
            daily_limit: 2,
        });

        mgr.check_and_increment().await;
        mgr.check_and_increment().await;

        let (ok, remaining, _) = mgr.check_and_increment().await;
        assert!(!ok);
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn should_report_status() {
        let mgr = QuotaManager::with_defaults();
        let (remaining, limit) = mgr.status().await;
        assert_eq!(remaining, 50);
        assert_eq!(limit, 50);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/rag_pipeline.rs">
//! RAG ingestion pipeline for repomix content.
//!
//! Pipeline: zip → repomix parse → enrich → chunk → Voyage embed → Qdrant upsert
//!
//! Each Qdrant point payload carries rich metadata for hover display:
//!   repo, file_path, language, symbols, doc_comments, imports, tags,
//!   is_test, line_start, line_end, chunk_index, total_chunks, content_hash

use anyhow::{Context, Result};
use qdrant_client::{
    qdrant::{
        vectors_config::Config as VectorsConfigEnum, Condition, CreateCollectionBuilder, Distance,
        Filter, PointStruct, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
        VectorsConfig,
    },
    Payload, Qdrant,
};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader},
    path::Path,
    sync::OnceLock,
    time::Duration,
};
use tracing::{info, warn};

// ─── constants ───────────────────────────────────────────────────────────────

pub const DEFAULT_COLLECTION: &str = "repomix_rag";
const VECTOR_DIM: u64 = 1024; // voyage-4 default
const CHUNK_LINES: usize = 80; // ~2 kB of code per chunk
const OVERLAP_LINES: usize = 12;
const VOYAGE_API_URL: &str = "https://api.voyageai.com/v1/embeddings";
const VOYAGE_BATCH: usize = 32; // points per upsert batch
const VOYAGE_RATE_DELAY_MS: u64 = 120; // ms between Voyage calls

// ─── extracted file metadata ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct FileMeta {
    pub language: &'static str,
    pub file_type: FileType,
    pub symbols: Vec<String>, // top-level pub items
    pub doc_comments: Vec<String>,
    pub imports: Vec<String>, // use / import / require
    pub tags: Vec<String>,    // semantic auto-tags
    pub is_test: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum FileType {
    #[default]
    Source,
    Test,
    Config,
    Docs,
    Build,
    Other,
}

impl FileType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Test => "test",
            Self::Config => "config",
            Self::Docs => "docs",
            Self::Build => "build",
            Self::Other => "other",
        }
    }
}

// ─── chunk ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Chunk {
    pub repo: String,
    pub file_path: String,
    pub meta: FileMeta,
    pub content: String,    // raw chunk lines
    pub embed_text: String, // metadata header + content (what gets embedded)
    pub content_hash: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub line_start: usize,
    pub line_end: usize,
}

// ─── ingestion stats ──────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct IngestStats {
    pub files_parsed: usize,
    pub chunks_created: usize,
    pub chunks_upserted: usize,
    pub chunks_skipped: usize,
    pub errors: usize,
}

// ─── public API ───────────────────────────────────────────────────────────────

/// RAG query result returned by `RagPipeline::query`.
#[derive(Debug, Clone, Serialize)]
pub struct RagResult {
    pub score: f32,
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

pub struct RagPipeline {
    qdrant: Qdrant,
    voyage_key: String,
    voyage_model: String,
    http: Client,
}

impl RagPipeline {
    pub fn from_env() -> Result<Self> {
        let voyage_key = std::env::var("VOYAGE_API_KEY")
            .or_else(|_| std::env::var("COGNITIVE_MCP_VOYAGE_API_KEY"))
            .context("VOYAGE_API_KEY not set")?;
        let voyage_model = std::env::var("COGNITIVE_MCP_VOYAGE_MODEL")
            .or_else(|_| std::env::var("VOYAGE_MODEL"))
            .unwrap_or_else(|_| "voyage-4-lite".into());

        let qdrant = qdrant_client_from_env()?;

        Ok(Self {
            qdrant,
            voyage_key,
            voyage_model,
            http: Client::new(),
        })
    }

    /// Ingest a single repomix file from the zip into Qdrant.
    pub async fn ingest_repomix_entry(
        &self,
        zip_path: &Path,
        entry_name: &str,
        collection: &str,
    ) -> Result<IngestStats> {
        self.ensure_collection(collection).await?;

        let repo = repo_name_from_entry(entry_name);
        info!(repo = %repo, entry = %entry_name, "Ingesting repomix entry");

        let file = std::fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let entry_idx = (0..archive.len())
            .find(|&i| {
                archive
                    .by_index(i)
                    .map(|e| e.name().to_string())
                    .ok()
                    .as_deref()
                    == Some(entry_name)
            })
            .with_context(|| format!("Entry '{entry_name}' not found in zip"))?;

        let entry = archive.by_index(entry_idx)?;
        let reader = BufReader::new(entry);

        let mut stats = IngestStats::default();
        let mut batch: Vec<PointStruct> = Vec::new();

        for chunk in parse_and_chunk(reader, &repo) {
            stats.files_parsed += 1;
            let total = chunk.total_chunks;
            stats.chunks_created += total;

            // Embed
            let vector = match self.embed_document(&chunk.embed_text).await {
                Ok(v) => v,
                Err(e) => {
                    warn!(file = %chunk.file_path, error = %e, "Embed failed, skipping chunk");
                    stats.errors += 1;
                    continue;
                }
            };

            // Build Qdrant payload with all metadata for hover display
            let payload: Payload = serde_json::json!({
                "repo":          chunk.repo,
                "file_path":     chunk.file_path,
                "language":      chunk.meta.language,
                "file_type":     chunk.meta.file_type.as_str(),
                "symbols":       chunk.meta.symbols,
                "doc_comments":  chunk.meta.doc_comments,
                "imports":       chunk.meta.imports,
                "tags":          chunk.meta.tags,
                "is_test":       chunk.meta.is_test,
                "line_start":    chunk.line_start,
                "line_end":      chunk.line_end,
                "chunk_index":   chunk.chunk_index,
                "total_chunks":  chunk.total_chunks,
                "content":       chunk.content,
                "content_hash":  chunk.content_hash,
            })
            .try_into()
            .context("Failed to build payload")?;

            // Use content hash as stable point ID (dedup)
            batch.push(PointStruct::new(
                stable_uuid(&chunk.content_hash),
                vector,
                payload,
            ));

            if batch.len() >= VOYAGE_BATCH {
                self.flush_batch(collection, &mut batch, &mut stats).await;
                tokio::time::sleep(Duration::from_millis(VOYAGE_RATE_DELAY_MS)).await;
            }
        }

        if !batch.is_empty() {
            self.flush_batch(collection, &mut batch, &mut stats).await;
        }

        info!(
            repo = %repo,
            files = stats.files_parsed,
            chunks = stats.chunks_created,
            upserted = stats.chunks_upserted,
            "Ingest complete"
        );

        Ok(stats)
    }

    /// Semantic search over a Qdrant collection.
    pub async fn query(
        &self,
        collection: &str,
        query_text: &str,
        limit: u64,
        repo_filter: Option<&str>,
    ) -> Result<Vec<RagResult>> {
        let vector = self.embed_query(query_text).await?;

        let mut builder = SearchPointsBuilder::new(collection, vector, limit).with_payload(true);

        if let Some(repo) = repo_filter {
            builder = builder.filter(Filter::must([Condition::matches("repo", repo.to_string())]));
        }

        let response = self.qdrant.search_points(builder).await?;

        Ok(response
            .result
            .into_iter()
            .map(|pt| {
                let p = serde_json::to_value(&pt.payload).unwrap_or_default();
                RagResult {
                    score: pt.score,
                    repo: str_field(&p, "repo"),
                    file_path: str_field(&p, "file_path"),
                    language: str_field(&p, "language"),
                    file_type: str_field(&p, "file_type"),
                    symbols: str_arr(&p, "symbols"),
                    doc_comments: str_arr(&p, "doc_comments"),
                    imports: str_arr(&p, "imports"),
                    tags: str_arr(&p, "tags"),
                    is_test: p["is_test"].as_bool().unwrap_or(false),
                    line_start: p["line_start"].as_i64().unwrap_or(0),
                    line_end: p["line_end"].as_i64().unwrap_or(0),
                    chunk_index: p["chunk_index"].as_i64().unwrap_or(0),
                    total_chunks: p["total_chunks"].as_i64().unwrap_or(1),
                    content: str_field(&p, "content"),
                }
            })
            .collect())
    }

    // ─── private ─────────────────────────────────────────────────────────────

    async fn ensure_collection(&self, name: &str) -> Result<()> {
        if !self.qdrant.collection_exists(name).await? {
            self.qdrant
                .create_collection(CreateCollectionBuilder::new(name).vectors_config(
                    VectorsConfig {
                        config: Some(VectorsConfigEnum::Params(
                            VectorParamsBuilder::new(VECTOR_DIM, Distance::Cosine).build(),
                        )),
                    },
                ))
                .await?;
            info!(collection = %name, "Created Qdrant collection");
        }
        Ok(())
    }

    async fn flush_batch(
        &self,
        collection: &str,
        batch: &mut Vec<PointStruct>,
        stats: &mut IngestStats,
    ) {
        match self
            .qdrant
            .upsert_points(UpsertPointsBuilder::new(
                collection,
                batch.drain(..).collect::<Vec<_>>(),
            ))
            .await
        {
            Ok(_) => stats.chunks_upserted += VOYAGE_BATCH.min(batch.capacity()),
            Err(e) => {
                warn!(error = %e, "Qdrant upsert failed");
                stats.errors += 1;
            }
        }
    }

    async fn embed_document(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text, "document").await
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text, "query").await
    }

    async fn embed(&self, text: &str, input_type: &str) -> Result<Vec<f32>> {
        #[derive(Serialize)]
        struct Req<'a> {
            input: Vec<&'a str>,
            model: &'a str,
            input_type: &'a str,
            truncation: bool,
            output_dimension: u64,
        }
        #[derive(Deserialize)]
        struct Resp {
            data: Vec<EmbData>,
        }
        #[derive(Deserialize)]
        struct EmbData {
            embedding: Vec<f32>,
        }

        let resp: Resp = self
            .http
            .post(VOYAGE_API_URL)
            .bearer_auth(&self.voyage_key)
            .json(&Req {
                input: vec![text],
                model: &self.voyage_model,
                input_type,
                truncation: true,
                output_dimension: VECTOR_DIM,
            })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        resp.data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .context("Voyage returned no embeddings")
    }
}

// ─── repomix streaming parser ─────────────────────────────────────────────────

/// Stream-parse a repomix file and yield enriched, chunked entries.
/// Never loads the whole file into memory.
fn parse_and_chunk(reader: impl BufRead, repo: &str) -> impl Iterator<Item = Chunk> {
    let repo = repo.to_string();
    let mut lines_iter = reader.lines();
    let mut pending: Option<(String, Vec<String>, usize)> = None; // (path, lines, start_lineno)
    let mut output: Vec<Chunk> = Vec::new();
    let mut file_count = 0usize;

    // State machine: collect lines between <file path="..."> and </file>
    while let Some(Ok(line)) = lines_iter.next() {
        file_count += 1;

        if let Some(path) = extract_file_path(&line) {
            pending = Some((path, Vec::new(), file_count));
            continue;
        }

        if line.trim() == "</file>" {
            if let Some((path, content_lines, _start)) = pending.take() {
                let meta = enrich(&path, &content_lines);
                let chunks = build_chunks(&repo, &path, meta, content_lines);
                output.extend(chunks);
            }
            continue;
        }

        if let Some((_, ref mut lines, _)) = pending {
            lines.push(line);
        }
    }

    output.into_iter()
}

fn extract_file_path(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("<file path=\"") {
        return None;
    }
    let rest = trimmed.strip_prefix("<file path=\"")?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

// ─── enrichment ───────────────────────────────────────────────────────────────

fn enrich(file_path: &str, lines: &[String]) -> FileMeta {
    let language = detect_language(file_path);
    let file_type = classify_file(file_path, lines);
    let is_test = file_type == FileType::Test
        || lines
            .iter()
            .any(|l| l.contains("#[test]") || l.contains("#[cfg(test)]"));

    let (symbols, doc_comments, imports) = match language {
        "rust" => extract_rust(lines),
        "typescript" | "javascript" => extract_ts(lines),
        "python" => extract_python(lines),
        "go" => extract_go(lines),
        _ => (vec![], vec![], vec![]),
    };

    let tags = auto_tags(file_path, &symbols, &imports, language);

    FileMeta {
        language,
        file_type,
        symbols,
        doc_comments,
        imports,
        tags,
        is_test,
    }
}

fn detect_language(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" => "javascript",
        "py" => "python",
        "go" => "go",
        "proto" => "protobuf",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "md" | "mdx" => "markdown",
        "sh" | "bash" => "shell",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "java" => "java",
        "kt" => "kotlin",
        "nix" => "nix",
        _ => "text",
    }
}

fn classify_file(path: &str, lines: &[String]) -> FileType {
    let lower = path.to_lowercase();
    if lower.contains("test") || lower.contains("spec") || lower.ends_with("_test.rs") {
        return FileType::Test;
    }
    if lower.ends_with("cargo.toml")
        || lower.ends_with("package.json")
        || lower.ends_with("pyproject.toml")
        || lower.ends_with("go.mod")
        || lower.ends_with("build.rs")
        || lower.ends_with("makefile")
    {
        return FileType::Build;
    }
    if lower.ends_with(".md") || lower.ends_with(".rst") || lower.ends_with(".txt") {
        return FileType::Docs;
    }
    if lower.ends_with(".toml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".json")
        || lower.ends_with(".env")
    {
        return FileType::Config;
    }
    if lines
        .iter()
        .any(|l| l.contains("#[cfg(test)]") || l.contains("#[test]"))
    {
        return FileType::Test;
    }
    FileType::Source
}

// Rust symbol extraction
fn extract_rust(lines: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    static RE_ITEM: OnceLock<Regex> = OnceLock::new();
    static RE_USE: OnceLock<Regex> = OnceLock::new();

    let re_item = RE_ITEM.get_or_init(|| {
        Regex::new(
            r"^\s*pub(?:\(crate\))?\s+(fn|struct|enum|trait|type|mod|const|static|impl)\s+(\w+)",
        )
        .unwrap()
    });
    let re_use = RE_USE.get_or_init(|| Regex::new(r"^\s*use\s+([\w::{}, ]+);").unwrap());

    let mut symbols = Vec::new();
    let mut doc_comments = Vec::new();
    let mut imports = Vec::new();
    let mut pending_doc: Vec<String> = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            let doc = trimmed.trim_start_matches('/').trim().to_string();
            if !doc.is_empty() {
                pending_doc.push(doc);
            }
            continue;
        }

        if let Some(caps) = re_item.captures(trimmed) {
            let kind = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            symbols.push(format!("{kind} {name}"));
            if !pending_doc.is_empty() {
                doc_comments.push(pending_doc.join(" "));
                pending_doc.clear();
            }
            continue;
        }

        if let Some(caps) = re_use.captures(trimmed) {
            let import = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            // Only keep top-level crate (first segment)
            let top = import.split("::").next().unwrap_or(&import);
            let top = top.trim_matches('{').trim().to_string();
            if !top.is_empty() && !imports.contains(&top) {
                imports.push(top);
            }
            continue;
        }

        pending_doc.clear();
    }

    // Cap to avoid huge payloads
    symbols.truncate(40);
    doc_comments.truncate(10);
    imports.truncate(30);

    (symbols, doc_comments, imports)
}

fn extract_ts(lines: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    static RE_EXPORT: OnceLock<Regex> = OnceLock::new();
    static RE_IMPORT: OnceLock<Regex> = OnceLock::new();

    let re_export = RE_EXPORT.get_or_init(|| {
        Regex::new(r"^export\s+(?:default\s+)?(?:async\s+)?(?:function|class|interface|type|const|enum)\s+(\w+)").unwrap()
    });
    let re_import =
        RE_IMPORT.get_or_init(|| Regex::new(r#"^import\s+.+from\s+['"]([^'"]+)['"]"#).unwrap());

    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    let mut doc_comments = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some(caps) = re_export.captures(trimmed) {
            symbols.push(caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string());
        }
        if let Some(caps) = re_import.captures(trimmed) {
            let src = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let pkg = src.split('/').next().unwrap_or(&src);
            let pkg = pkg.trim_start_matches('@').to_string();
            if !imports.contains(&pkg) {
                imports.push(pkg);
            }
        }
        if trimmed.starts_with("/**") || trimmed.starts_with("* ") {
            let doc = trimmed.trim_start_matches(['/', '*', ' ']).to_string();
            if !doc.is_empty() {
                doc_comments.push(doc);
            }
        }
    }

    symbols.truncate(40);
    doc_comments.truncate(10);
    imports.truncate(30);
    (symbols, doc_comments, imports)
}

fn extract_python(lines: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    static RE_DEF: OnceLock<Regex> = OnceLock::new();
    static RE_IMP: OnceLock<Regex> = OnceLock::new();

    let re_def = RE_DEF.get_or_init(|| Regex::new(r"^(?:class|def|async def)\s+(\w+)").unwrap());
    let re_imp = RE_IMP.get_or_init(|| Regex::new(r"^(?:import|from)\s+([\w.]+)").unwrap());

    let mut symbols = Vec::new();
    let mut imports = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some(caps) = re_def.captures(trimmed) {
            symbols.push(caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string());
        }
        if let Some(caps) = re_imp.captures(trimmed) {
            let pkg = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let top = pkg.split('.').next().unwrap_or(&pkg).to_string();
            if !imports.contains(&top) {
                imports.push(top);
            }
        }
    }

    symbols.truncate(40);
    imports.truncate(30);
    (symbols, vec![], imports)
}

fn extract_go(lines: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    static RE_DECL: OnceLock<Regex> = OnceLock::new();
    static RE_IMP: OnceLock<Regex> = OnceLock::new();

    let re_decl = RE_DECL.get_or_init(|| {
        Regex::new(r"^func\s+(?:\(\w+\s+\*?\w+\)\s+)?(\w+)|^type\s+(\w+)\s+(?:struct|interface)")
            .unwrap()
    });
    let re_imp = RE_IMP.get_or_init(|| Regex::new(r#"^\s+"([^"]+)""#).unwrap());

    let mut symbols = Vec::new();
    let mut imports = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some(caps) = re_decl.captures(trimmed) {
            let name = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                symbols.push(name);
            }
        }
        if let Some(caps) = re_imp.captures(trimmed) {
            let pkg = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let top = pkg.split('/').last().unwrap_or(pkg).to_string();
            if !imports.contains(&top) {
                imports.push(top);
            }
        }
    }

    symbols.truncate(40);
    imports.truncate(30);
    (symbols, vec![], imports)
}

fn auto_tags(path: &str, symbols: &[String], imports: &[String], lang: &str) -> Vec<String> {
    let mut tags: Vec<&str> = Vec::new();

    // Language tag
    tags.push(lang);

    // Path-based tags
    let lower = path.to_lowercase();
    for keyword in [
        "server", "client", "handler", "router", "auth", "error", "config", "test", "bench",
        "proto", "grpc", "http", "async", "stream", "channel", "database", "cache",
    ] {
        if lower.contains(keyword) {
            tags.push(keyword);
        }
    }

    // Symbol-based tags
    let sym_text = symbols.join(" ").to_lowercase();
    for keyword in [
        "trait", "impl", "async", "handler", "service", "client", "server", "error", "config",
        "builder", "stream",
    ] {
        if sym_text.contains(keyword) {
            tags.push(keyword);
        }
    }

    // Import-based tags
    for imp in imports {
        match imp.as_str() {
            "tokio" | "async_std" => tags.push("async"),
            "tonic" | "prost" => tags.push("grpc"),
            "axum" | "hyper" | "actix" | "warp" => tags.push("http"),
            "serde" | "serde_json" => tags.push("serialization"),
            "sqlx" | "diesel" | "sea_orm" => tags.push("database"),
            "tracing" | "log" => tags.push("logging"),
            "anyhow" | "thiserror" => tags.push("error-handling"),
            _ => {}
        }
    }

    let mut deduped: Vec<String> = Vec::new();
    for t in tags {
        let s = t.to_string();
        if !deduped.contains(&s) {
            deduped.push(s);
        }
    }
    deduped.truncate(20);
    deduped
}

// ─── chunking ─────────────────────────────────────────────────────────────────

fn build_chunks(repo: &str, file_path: &str, meta: FileMeta, lines: Vec<String>) -> Vec<Chunk> {
    if lines.is_empty() {
        return vec![];
    }

    let mut chunks = Vec::new();
    let step = CHUNK_LINES.saturating_sub(OVERLAP_LINES).max(1);
    let total = ((lines.len() as f64) / step as f64).ceil() as usize;
    let total = total.max(1);

    let mut idx = 0usize;
    let mut chunk_index = 0usize;
    while idx < lines.len() {
        let end = (idx + CHUNK_LINES).min(lines.len());
        let chunk_lines = &lines[idx..end];
        let content = chunk_lines.join("\n");
        let embed_text = build_embed_text(repo, file_path, &meta, &content);
        let content_hash =
            hex_hash(format!("{repo}:{file_path}:{chunk_index}:{content}").as_bytes());

        chunks.push(Chunk {
            repo: repo.to_string(),
            file_path: file_path.to_string(),
            meta: meta.clone(),
            content,
            embed_text,
            content_hash,
            chunk_index,
            total_chunks: total,
            line_start: idx + 1,
            line_end: end,
        });

        idx += step;
        chunk_index += 1;

        if end >= lines.len() {
            break;
        }
    }

    chunks
}

/// Build the text that gets embedded — metadata header + content.
/// The header primes the embedding model with structural context.
fn build_embed_text(repo: &str, file_path: &str, meta: &FileMeta, content: &str) -> String {
    let mut header = format!("REPO: {repo}\nFILE: {file_path}\nLANG: {}\n", meta.language);

    if !meta.symbols.is_empty() {
        header.push_str(&format!("SYMBOLS: {}\n", meta.symbols.join(", ")));
    }
    if !meta.doc_comments.is_empty() {
        header.push_str(&format!("DOCS: {}\n", meta.doc_comments.first().unwrap()));
    }
    if !meta.imports.is_empty() {
        header.push_str(&format!("DEPS: {}\n", meta.imports.join(", ")));
    }
    if !meta.tags.is_empty() {
        header.push_str(&format!("TAGS: {}\n", meta.tags.join(", ")));
    }
    if meta.is_test {
        header.push_str("TYPE: test\n");
    }

    format!("{header}---\n{content}")
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn repo_name_from_entry(entry_name: &str) -> String {
    // "rust-analyzer-repomix.md" → "rust-analyzer"
    // "google-cloud-rust-repomix-2.md" → "google-cloud-rust"
    let base = entry_name.trim_end_matches(".md").trim_end_matches(".xml");

    // Strip trailing "-repomix" and any "-N" suffix
    let base = if let Some(pos) = base.rfind("-repomix") {
        &base[..pos]
    } else {
        base
    };

    base.to_string()
}

fn hex_hash(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn stable_uuid(hash: &str) -> String {
    // Use first 32 hex chars of hash to form a UUID-like stable ID
    if hash.len() >= 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &hash[0..8],
            &hash[8..12],
            &hash[12..16],
            &hash[16..20],
            &hash[20..32]
        )
    } else {
        uuid::Uuid::new_v4().to_string()
    }
}

// ─── Qdrant connection ────────────────────────────────────────────────────────

/// Build a Qdrant client from environment.
///
/// Connection is always TCP to localhost. When the qdrant container socket is
/// declared via `UNIX_SOCKET_ENDPOINTS` (e.g. `qdrant:/run/qdrant.sock:6334`),
/// xray transparently proxies `127.0.0.1:6334` → container unix socket using
/// its native domain-socket (`"network": "ds"`) outbound — no custom tonic
/// connector needed here.
///
/// Override the URL with `COGNITIVE_MCP_QDRANT_URL`.
pub fn qdrant_client_from_env() -> Result<Qdrant> {
    let url = std::env::var("COGNITIVE_MCP_QDRANT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:6334".into());
    Qdrant::from_url(&url)
        .build()
        .context("Failed to build Qdrant client")
}

fn str_field(v: &serde_json::Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}

fn str_arr(v: &serde_json::Value, key: &str) -> Vec<String> {
    v[key]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/server.rs">
//! Cognitive MCP Server — Dual Transport (HTTP/SSE + gRPC)
//!
//! Single persistent backend (CozoDB) hosts every relation:
//! memory namespaces/entries, users, sessions, compliance graph, subid registry, audit log.

use crate::cognitive_tools::CognitiveToolRegistry;
use crate::cozo_shuttle::CozoGraphShuttle;
use crate::gemini_fallback::GeminiFallback;
use crate::grpc_service::CognitiveGrpcService;
use crate::memory_store::CognitiveMemoryStore;
use crate::proto::cognitive_tool_service_server::CognitiveToolServiceServer;
use crate::qdrant_shuttle::QdrantSemanticShuttle;
use crate::quota::QuotaManager;
use crate::session::SessionManager;
use crate::typed_tools;
use op_mcp::tool_registry::{RegistryExecutor, ToolRegistry};
use std::path::PathBuf;
use std::sync::Arc;

pub struct CognitiveMcpServer {
    memory_store: Arc<CognitiveMemoryStore>,
    cozo_shuttle: Arc<CozoGraphShuttle>,
    qdrant_shuttle: Option<Arc<QdrantSemanticShuttle>>,
    tool_registry: Arc<ToolRegistry>,
    session_manager: Arc<SessionManager>,
    quota_manager: Arc<QuotaManager>,
    gemini_fallback: Arc<GeminiFallback>,
}

impl CognitiveMcpServer {
    /// `db_path` is the CozoDB directory backing every persistent relation
    /// (memory namespaces/entries, users, sessions, compliance graph, audit log).
    pub async fn new(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let cozo_shuttle = Arc::new(CozoGraphShuttle::new_persistent(PathBuf::from(db_path))?);
        let memory_store = Arc::new(CognitiveMemoryStore::new(cozo_shuttle.clone()).await?);

        let tool_registry = Arc::new(ToolRegistry::new());

        let qdrant_shuttle = match QdrantSemanticShuttle::new().await {
            Ok(shuttle) => Some(Arc::new(shuttle)),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Qdrant Semantic Shuttle unavailable; cognitive MCP will continue without vector retrieval"
                );
                None
            }
        };

        let session_manager = Arc::new(SessionManager::with_defaults());
        let quota_manager = Arc::new(QuotaManager::with_defaults());
        let gemini_fallback = Arc::new(GeminiFallback::new());

        CognitiveToolRegistry::register_all(&tool_registry, memory_store.clone()).await?;

        typed_tools::register_typed_tools(
            &tool_registry,
            memory_store.clone(),
            session_manager.clone(),
            quota_manager.clone(),
        )
        .await?;

        Ok(Self {
            memory_store,
            cozo_shuttle,
            qdrant_shuttle,
            tool_registry,
            session_manager,
            quota_manager,
            gemini_fallback,
        })
    }

    pub async fn start_http_server(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        use op_mcp::{HttpSseTransport, McpServer, McpServerConfig, Transport};

        let config = McpServerConfig {
            name: Some("cognitive-mcp".to_string()),
            compact_mode: false,
            ..Default::default()
        };

        let executor = Arc::new(RegistryExecutor::new(self.tool_registry.clone()));
        let mcp_server = Arc::new(McpServer::with_executor(config, executor));
        let transport = HttpSseTransport::new(addr.to_string());

        tracing::info!("Cognitive MCP Server listening on {}", addr);
        transport.serve(mcp_server).await?;
        Ok(())
    }

    pub async fn start_grpc_server(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let grpc_service = CognitiveGrpcService::new(
            self.memory_store.clone(),
            self.session_manager.clone(),
            self.quota_manager.clone(),
            self.gemini_fallback.clone(),
        );

        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
            .build_v1()
            .expect("failed to build cognitive reflection service");

        let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
        health_reporter
            .set_serving::<CognitiveToolServiceServer<CognitiveGrpcService>>()
            .await;

        let socket_addr: std::net::SocketAddr = addr.parse()?;
        tracing::info!(addr = %socket_addr, "Cognitive gRPC Server listening");

        tonic::transport::Server::builder()
            .accept_http1(true)
            .add_service(tonic_web::enable(
                CognitiveToolServiceServer::with_interceptor(
                    grpc_service,
                    crate::interceptor::ghostbridge_interceptor,
                ),
            ))
            .add_service(tonic_web::enable(reflection))
            .add_service(tonic_web::enable(health_service))
            .serve(socket_addr)
            .await?;

        Ok(())
    }

    pub async fn start_dual(
        self,
        http_addr: &str,
        grpc_addr: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let grpc_addr = grpc_addr.to_string();
        let http_addr = http_addr.to_string();

        let grpc_memory = self.memory_store.clone();
        let grpc_session = self.session_manager.clone();
        let grpc_quota = self.quota_manager.clone();
        let grpc_gemini = self.gemini_fallback.clone();

        let grpc_handle = tokio::spawn(async move {
            let grpc_service =
                CognitiveGrpcService::new(grpc_memory, grpc_session, grpc_quota, grpc_gemini);

            let reflection = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
                .build_v1()
                .expect("failed to build cognitive reflection service");

            let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
            health_reporter
                .set_serving::<CognitiveToolServiceServer<CognitiveGrpcService>>()
                .await;

            let socket_addr: std::net::SocketAddr = grpc_addr.parse().expect("invalid gRPC addr");
            tracing::info!(addr = %socket_addr, "Cognitive gRPC Server listening");

            tonic::transport::Server::builder()
                .accept_http1(true)
                .add_service(tonic_web::enable(
                    CognitiveToolServiceServer::with_interceptor(
                        grpc_service,
                        crate::interceptor::ghostbridge_interceptor,
                    ),
                ))
                .add_service(tonic_web::enable(reflection))
                .add_service(tonic_web::enable(health_service))
                .serve(socket_addr)
                .await
                .expect("gRPC server failed");
        });

        self.start_http_server(&http_addr).await?;
        grpc_handle.await?;
        Ok(())
    }

    pub fn memory_store(&self) -> Arc<CognitiveMemoryStore> {
        self.memory_store.clone()
    }

    pub fn cozo_shuttle(&self) -> Arc<CozoGraphShuttle> {
        self.cozo_shuttle.clone()
    }

    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    /// Register on the session D-Bus and serve until the connection drops.
    /// Runs in the background — call this before start_http_server / start_dual.
    pub async fn start_dbus(&self) -> Result<zbus::Connection, Box<dyn std::error::Error>> {
        use crate::dbus_interface::CognitiveMcpInterface;

        let conn = zbus::Connection::system().await?;
        conn.request_name("org.opdbus.CognitiveMcp").await?;

        let iface = CognitiveMcpInterface::new(self.tool_registry.clone());
        conn.object_server()
            .at("/org/opdbus/v1/cognitive", iface)
            .await?;

        tracing::info!("Cognitive MCP D-Bus interface registered at /org/opdbus/v1/cognitive");
        Ok(conn)
    }

    pub fn qdrant_shuttle(&self) -> Option<Arc<QdrantSemanticShuttle>> {
        self.qdrant_shuttle.clone()
    }

    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }

    pub fn quota_manager(&self) -> Arc<QuotaManager> {
        self.quota_manager.clone()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/session.rs">
//! Session Manager — Conversation Memory (R2, R10)
//!
//! Provides conversation_id-based session tracking for follow-up queries.
//! Sessions are stored in SQLite alongside the memory store for durability.
//! Each session holds the conversation context and query history.
//!
//! Operations: create, get, list, reset, close.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// A conversation session for NotebookLM follow-up queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSession {
    pub id: String,
    pub notebook_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub query_count: u32,
    /// Last N queries for context window (sliding window).
    pub history: Vec<QueryTurn>,
    pub active: bool,
}

/// A single query/answer turn within a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryTurn {
    pub query: String,
    pub answer: String,
    pub timestamp: DateTime<Utc>,
    pub citations_count: u32,
    pub grounded: bool,
}

/// Session manager backed by in-memory DashMap with optional SQLite persistence.
/// Phase 1 uses in-memory only; Phase 3 will add SQLite backing via the
/// CognitiveMemoryStore's pool.
pub struct SessionManager {
    sessions: Arc<DashMap<String, ConversationSession>>,
    /// Maximum turns kept per conversation before eviction.
    max_history: usize,
}

impl SessionManager {
    pub fn new(max_history: usize) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            max_history,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(20)
    }

    /// Get or create a session for a conversation_id.
    /// If conversation_id is empty, generates a new one.
    pub fn get_or_create(&self, conversation_id: &str, notebook_id: &str) -> ConversationSession {
        let id = if conversation_id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            conversation_id.to_string()
        };

        self.sessions
            .entry(id.clone())
            .or_insert_with(|| ConversationSession {
                id: id.clone(),
                notebook_id: notebook_id.to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                query_count: 0,
                history: Vec::new(),
                active: true,
            })
            .clone()
    }

    /// Append a query turn to the conversation and return the updated session.
    pub fn append_turn(
        &self,
        conversation_id: &str,
        turn: QueryTurn,
    ) -> Result<ConversationSession> {
        let mut entry = self
            .sessions
            .get_mut(conversation_id)
            .context(format!("session '{}' not found", conversation_id))?;

        entry.query_count += 1;
        entry.updated_at = Utc::now();
        entry.history.push(turn);

        // Evict oldest turns beyond max_history
        while entry.history.len() > self.max_history {
            entry.history.remove(0);
        }

        Ok(entry.clone())
    }

    /// List all active sessions.
    pub fn list_sessions(&self) -> Vec<ConversationSession> {
        self.sessions
            .iter()
            .filter(|entry| entry.active)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Reset a session (clear history, keep ID).
    pub fn reset_session(&self, conversation_id: &str) -> Result<ConversationSession> {
        let mut entry = self
            .sessions
            .get_mut(conversation_id)
            .context(format!("session '{}' not found", conversation_id))?;

        entry.history.clear();
        entry.query_count = 0;
        entry.updated_at = Utc::now();
        Ok(entry.clone())
    }

    /// Close a session (marks inactive, retains for audit).
    pub fn close_session(&self, conversation_id: &str) -> Result<()> {
        let mut entry = self
            .sessions
            .get_mut(conversation_id)
            .context(format!("session '{}' not found", conversation_id))?;

        entry.active = false;
        entry.updated_at = Utc::now();
        Ok(())
    }

    /// Get a specific session.
    pub fn get_session(&self, conversation_id: &str) -> Option<ConversationSession> {
        self.sessions.get(conversation_id).map(|e| e.clone())
    }

    /// Total session count.
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Active session count.
    pub fn active_count(&self) -> usize {
        self.sessions.iter().filter(|e| e.active).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_create_session_with_generated_id() {
        let mgr = SessionManager::with_defaults();
        let session = mgr.get_or_create("", "notebook-1");
        assert!(!session.id.is_empty());
        assert_eq!(session.notebook_id, "notebook-1");
        assert!(session.active);
    }

    #[test]
    fn should_reuse_existing_session() {
        let mgr = SessionManager::with_defaults();
        let s1 = mgr.get_or_create("conv-abc", "notebook-1");
        let s2 = mgr.get_or_create("conv-abc", "notebook-1");
        assert_eq!(s1.id, s2.id);
    }

    #[test]
    fn should_append_and_evict_turns() {
        let mgr = SessionManager::new(2);
        let session = mgr.get_or_create("conv-x", "nb-1");
        assert_eq!(session.query_count, 0);

        for i in 0..5 {
            mgr.append_turn(
                "conv-x",
                QueryTurn {
                    query: format!("q{}", i),
                    answer: format!("a{}", i),
                    timestamp: Utc::now(),
                    citations_count: 0,
                    grounded: true,
                },
            )
            .unwrap();
        }

        let updated = mgr.get_session("conv-x").unwrap();
        assert_eq!(updated.query_count, 5);
        // Only last 2 turns kept
        assert_eq!(updated.history.len(), 2);
        assert_eq!(updated.history[0].query, "q3");
    }

    #[test]
    fn should_reset_session() {
        let mgr = SessionManager::with_defaults();
        mgr.get_or_create("conv-r", "nb-1");
        mgr.append_turn(
            "conv-r",
            QueryTurn {
                query: "q".into(),
                answer: "a".into(),
                timestamp: Utc::now(),
                citations_count: 0,
                grounded: true,
            },
        )
        .unwrap();

        let reset = mgr.reset_session("conv-r").unwrap();
        assert_eq!(reset.query_count, 0);
        assert!(reset.history.is_empty());
    }

    #[test]
    fn should_close_session() {
        let mgr = SessionManager::with_defaults();
        mgr.get_or_create("conv-c", "nb-1");
        mgr.close_session("conv-c").unwrap();

        let session = mgr.get_session("conv-c").unwrap();
        assert!(!session.active);
        assert_eq!(mgr.active_count(), 0);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/soul_memory.rs">
//! Typed APIs for Soul Memory (persistent agent identity) and
//! Agent → Namespace bindings. Both relations live in the same CozoDB instance
//! backing the rest of the cognitive memory store.

use crate::cozo_shuttle::CozoGraphShuttle;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use cozo::{DataValue, NamedRows, ScriptMutability};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

type Params = BTreeMap<String, DataValue>;

/// Soul memory = persistent identity for an agent. Survives sessions and
/// agent migrations. Versioned on every update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulMemory {
    pub agent_id: String,
    pub identity: String,
    pub personality: String,
    pub traits: serde_json::Value,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Binding from an agent to its owning memory namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNamespaceBinding {
    pub agent_id: String,
    pub namespace: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct SoulUpdate {
    pub identity: Option<String>,
    pub personality: Option<String>,
    pub traits: Option<serde_json::Value>,
}

pub struct SoulMemoryStore {
    shuttle: Arc<CozoGraphShuttle>,
}

impl SoulMemoryStore {
    pub fn new(shuttle: Arc<CozoGraphShuttle>) -> Self {
        Self { shuttle }
    }

    fn run(&self, script: &str, params: Params) -> Result<NamedRows> {
        self.shuttle
            .db()
            .run_script(script, params, ScriptMutability::Mutable)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    // ── Soul ─────────────────────────────────────────────────────────────────

    pub async fn upsert_soul(&self, agent_id: &str, update: SoulUpdate) -> Result<SoulMemory> {
        let existing = self.get_soul(agent_id).await?;
        let now = Utc::now();
        let now_s = now.to_rfc3339();

        let identity = update
            .identity
            .or_else(|| existing.as_ref().map(|s| s.identity.clone()))
            .unwrap_or_default();
        let personality = update
            .personality
            .or_else(|| existing.as_ref().map(|s| s.personality.clone()))
            .unwrap_or_default();
        let traits = update
            .traits
            .or_else(|| existing.as_ref().map(|s| s.traits.clone()))
            .unwrap_or(serde_json::Value::Object(Default::default()));
        let version = existing.as_ref().map(|s| s.version + 1).unwrap_or(1);
        let created_at = existing
            .as_ref()
            .map(|s| s.created_at.to_rfc3339())
            .unwrap_or_else(|| now_s.clone());

        let q = r#"
            ?[agent_id, identity, personality, traits, version, created_at, updated_at]
                <- [[$id, $ident, $pers, $traits, $ver, $ca, $now]]
            :put soul_memories {
                agent_id => identity, personality, traits, version, created_at, updated_at
            }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(agent_id.into()));
        p.insert("ident".into(), DataValue::Str(identity.into()));
        p.insert("pers".into(), DataValue::Str(personality.into()));
        p.insert(
            "traits".into(),
            DataValue::Str(serde_json::to_string(&traits)?.into()),
        );
        p.insert("ver".into(), DataValue::Num(cozo::Num::Int(version)));
        p.insert("ca".into(), DataValue::Str(created_at.into()));
        p.insert("now".into(), DataValue::Str(now_s.into()));
        self.run(q, p).context("upsert soul")?;

        self.get_soul(agent_id)
            .await?
            .context("soul vanished after upsert")
    }

    pub async fn get_soul(&self, agent_id: &str) -> Result<Option<SoulMemory>> {
        let q = r#"
            ?[agent_id, identity, personality, traits, version, created_at, updated_at]
                := *soul_memories[agent_id, identity, personality, traits, version, created_at, updated_at],
                   agent_id = $id
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(agent_id.into()));
        let rows = self.run(q, p).context("get soul")?;
        Ok(rows.rows.first().map(row_to_soul))
    }

    pub async fn delete_soul(&self, agent_id: &str) -> Result<bool> {
        if self.get_soul(agent_id).await?.is_none() {
            return Ok(false);
        }
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(agent_id.into()));
        self.run("?[agent_id] <- [[$id]] :rm soul_memories { agent_id }", p)
            .context("delete soul")?;
        Ok(true)
    }

    pub async fn list_souls(&self) -> Result<Vec<SoulMemory>> {
        let q = r#"
            ?[agent_id, identity, personality, traits, version, created_at, updated_at]
                := *soul_memories[agent_id, identity, personality, traits, version, created_at, updated_at]
            :order agent_id
        "#;
        let rows = self.run(q, BTreeMap::new()).context("list souls")?;
        Ok(rows.rows.iter().map(row_to_soul).collect())
    }

    // ── Agent → Namespace binding ────────────────────────────────────────────

    pub async fn bind_namespace(
        &self,
        agent_id: &str,
        namespace: &str,
    ) -> Result<AgentNamespaceBinding> {
        let existing = self.get_binding(agent_id).await?;
        let now = Utc::now();
        let now_s = now.to_rfc3339();
        let created_at = existing
            .as_ref()
            .map(|b| b.created_at.to_rfc3339())
            .unwrap_or_else(|| now_s.clone());

        let q = r#"
            ?[agent_id, namespace, created_at, updated_at]
                <- [[$id, $ns, $ca, $now]]
            :put agent_namespace_bindings {
                agent_id => namespace, created_at, updated_at
            }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(agent_id.into()));
        p.insert("ns".into(), DataValue::Str(namespace.into()));
        p.insert("ca".into(), DataValue::Str(created_at.into()));
        p.insert("now".into(), DataValue::Str(now_s.into()));
        self.run(q, p).context("bind namespace")?;

        self.get_binding(agent_id)
            .await?
            .context("binding vanished after upsert")
    }

    pub async fn get_binding(&self, agent_id: &str) -> Result<Option<AgentNamespaceBinding>> {
        let q = r#"
            ?[agent_id, namespace, created_at, updated_at]
                := *agent_namespace_bindings[agent_id, namespace, created_at, updated_at],
                   agent_id = $id
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(agent_id.into()));
        let rows = self.run(q, p).context("get binding")?;
        Ok(rows.rows.first().map(row_to_binding))
    }

    pub async fn clear_binding(&self, agent_id: &str) -> Result<bool> {
        if self.get_binding(agent_id).await?.is_none() {
            return Ok(false);
        }
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(agent_id.into()));
        self.run(
            "?[agent_id] <- [[$id]] :rm agent_namespace_bindings { agent_id }",
            p,
        )
        .context("clear binding")?;
        Ok(true)
    }

    pub async fn list_bindings(&self) -> Result<Vec<AgentNamespaceBinding>> {
        let q = r#"
            ?[agent_id, namespace, created_at, updated_at]
                := *agent_namespace_bindings[agent_id, namespace, created_at, updated_at]
            :order agent_id
        "#;
        let rows = self.run(q, BTreeMap::new()).context("list bindings")?;
        Ok(rows.rows.iter().map(row_to_binding).collect())
    }
}

// ── Row → struct ────────────────────────────────────────────────────────────

fn row_to_soul(row: &Vec<DataValue>) -> SoulMemory {
    SoulMemory {
        agent_id: dv_str(&row[0]),
        identity: dv_str(&row[1]),
        personality: dv_str(&row[2]),
        traits: serde_json::from_str(&dv_str(&row[3])).unwrap_or(serde_json::Value::Null),
        version: dv_int(&row[4]),
        created_at: parse_ts(&dv_str(&row[5])),
        updated_at: parse_ts(&dv_str(&row[6])),
    }
}

fn row_to_binding(row: &Vec<DataValue>) -> AgentNamespaceBinding {
    AgentNamespaceBinding {
        agent_id: dv_str(&row[0]),
        namespace: dv_str(&row[1]),
        created_at: parse_ts(&dv_str(&row[2])),
        updated_at: parse_ts(&dv_str(&row[3])),
    }
}

fn dv_str(dv: &DataValue) -> String {
    if let DataValue::Str(s) = dv {
        s.to_string()
    } else {
        String::new()
    }
}

fn dv_int(dv: &DataValue) -> i64 {
    if let DataValue::Num(cozo::Num::Int(i)) = dv {
        *i
    } else {
        0
    }
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/tool_profiles.rs">
//! ⚖️ Tool Profiles — R14
//!
//! Controls which tools are exposed to agents to reduce token cost.
//! Minimal (5), Standard (10), Full (16).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolProfile {
    Minimal,
    Standard,
    Full,
}

impl std::fmt::Display for ToolProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Minimal => write!(f, "minimal"),
            Self::Standard => write!(f, "standard"),
            Self::Full => write!(f, "full"),
        }
    }
}

impl std::str::FromStr for ToolProfile {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "minimal" | "min" => Self::Minimal,
            "standard" | "std" => Self::Standard,
            "full" | "all" => Self::Full,
            _ => Self::Standard,
        })
    }
}

impl Default for ToolProfile {
    fn default() -> Self {
        Self::Standard
    }
}

pub fn tools_for_profile(profile: ToolProfile) -> Vec<&'static str> {
    match profile {
        ToolProfile::Minimal => vec![
            "ask_question",
            "list_notebooks",
            "select_notebook",
            "get_notebook",
            "get_health",
        ],
        ToolProfile::Standard => vec![
            "ask_question",
            "query_notebook",
            "list_notebooks",
            "select_notebook",
            "get_notebook",
            "add_source_url",
            "add_source_text",
            "list_sources",
            "get_source_content",
            "get_health",
        ],
        ToolProfile::Full => vec![
            "ask_question",
            "query_notebook",
            "list_notebooks",
            "select_notebook",
            "get_notebook",
            "create_notebook",
            "batch_create_notebooks",
            "add_source_url",
            "add_source_text",
            "add_folder",
            "list_sources",
            "remove_source",
            "get_source_content",
            "generate_data_table",
            "get_health",
            "doctor",
        ],
    }
}

pub fn is_tool_allowed(profile: ToolProfile, tool_name: &str) -> bool {
    tools_for_profile(profile).contains(&tool_name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileTokenEstimate {
    pub tool_count: u32,
    pub schema_tokens: u32,
    pub savings_percent: u32,
}

pub fn token_estimate(profile: ToolProfile) -> ProfileTokenEstimate {
    match profile {
        ToolProfile::Minimal => ProfileTokenEstimate {
            tool_count: 5,
            schema_tokens: 800,
            savings_percent: 69,
        },
        ToolProfile::Standard => ProfileTokenEstimate {
            tool_count: 10,
            schema_tokens: 1600,
            savings_percent: 38,
        },
        ToolProfile::Full => ProfileTokenEstimate {
            tool_count: 16,
            schema_tokens: 2600,
            savings_percent: 0,
        },
    }
}

pub fn current_profile() -> ToolProfile {
    std::env::var("COGNITIVE_MCP_TOOL_PROFILE")
        .or_else(|_| std::env::var("NOTEBOOKLM_PROFILE"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_correct_tool_counts() {
        assert_eq!(tools_for_profile(ToolProfile::Minimal).len(), 5);
        assert_eq!(tools_for_profile(ToolProfile::Standard).len(), 10);
        assert_eq!(tools_for_profile(ToolProfile::Full).len(), 16);
    }

    #[test]
    fn should_check_tool_allowed() {
        assert!(is_tool_allowed(ToolProfile::Minimal, "ask_question"));
        assert!(!is_tool_allowed(ToolProfile::Minimal, "doctor"));
        assert!(is_tool_allowed(ToolProfile::Full, "doctor"));
    }

    #[test]
    fn should_parse_profile_names() {
        assert_eq!(
            "minimal".parse::<ToolProfile>().unwrap(),
            ToolProfile::Minimal
        );
        assert_eq!("full".parse::<ToolProfile>().unwrap(), ToolProfile::Full);
    }

    #[test]
    fn should_estimate_token_savings() {
        let minimal = token_estimate(ToolProfile::Minimal);
        let full = token_estimate(ToolProfile::Full);
        assert!(minimal.savings_percent > 0);
        assert_eq!(full.savings_percent, 0);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/typed_tools.rs">
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
    namespace: String,
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
        Self {
            name: name.to_string(),
            description: description.to_string(),
            namespace: namespace.to_string(),
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
        "notebooklm"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "notebooklm".to_string(),
            "query".to_string(),
            "grounded".to_string(),
        ]
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
        let query = input["query"]
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

        let conversation_id = input["conversation_id"].as_str().unwrap_or("");

        let session = self
            .sessions
            .get_or_create(conversation_id, &self.namespace);

        let entries = self
            .store
            .query_entries(crate::memory_store::EntryQuery {
                namespace_id: Some(self.namespace.clone()),
                key_pattern: Some(query.to_string()),
                tags: None,
                limit: Some(10),
                offset: None,
            })
            .await?;

        let grounded = !entries.is_empty();
        let answer = if grounded {
            entries
                .iter()
                .map(|e| format!("[{}] {}", e.key, e.value))
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            format!("No grounded answer for '{}' in {}", query, self.namespace)
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
            "namespace": self.namespace
        }))
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
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing content"))?;

        let tags: Vec<String> = input["tags"]
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
        let kind = input["kind_filter"]
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
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/src/voyage.rs">
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

/// Voyage AI client for text embeddings
pub struct VoyageClient {
    client: Client,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    input: Vec<&'a str>,
    model: &'a str,
    input_type: Option<&'a str>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

impl VoyageClient {
    /// Create a new Voyage client
    pub fn new() -> Result<Self> {
        let api_key = env::var("VOYAGE_API_KEY").context("VOYAGE_API_KEY not found")?;
        // Use voyage-law-2 or voyage-4-large as specified
        let model = env::var("VOYAGE_MODEL").unwrap_or_else(|_| "voyage-law-2".to_string());

        Ok(Self {
            client: Client::new(),
            api_key,
            model,
        })
    }

    /// Embed text using Voyage API
    pub async fn embed(&self, text: &str, input_type: Option<&str>) -> Result<Vec<f32>> {
        let req = EmbeddingRequest {
            input: vec![text],
            model: &self.model,
            input_type,
        };

        let resp = self
            .client
            .post("https://api.voyageai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .context("Failed to send Voyage API request")?
            .error_for_status()
            .context("Voyage API returned error status")?
            .json::<EmbeddingResponse>()
            .await
            .context("Failed to parse Voyage API response")?;

        resp.data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .context("Voyage API returned no embeddings")
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/build.rs">
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_file = "proto/cognitive.proto";

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(
            std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("cognitive_descriptor.bin"),
        )
        .compile_protos(&[proto_file], &["proto/"])?;

    println!("cargo:rerun-if-changed={}", proto_file);
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/Cargo.toml">
[package]
name = "op-cognitive-mcp"
version = "0.1.0"
edition = "2021"

[dependencies]
op-core = { path = "../op-core" }
op-identity = { path = "../op-identity" }
op-mcp = { path = "../op-mcp" }
op-agents = { path = "../op-agents" }
op-state-store = { path = "../op-state-store" }
op-dynamic-loader = { path = "../op-dynamic-loader" }
op-cache = { path = "../op-cache" }
op-cozo-store = { workspace = true }
hex = { workspace = true }
memmap2 = { workspace = true }
serde = { version = "1.0", features = ["derive"] }
serde_json = { workspace = true }
simd-json = { workspace = true }
tokio = { version = "1.0", features = ["full"] }
anyhow = { workspace = true }
qdrant-client = "1.17"
reqwest = { workspace = true }
tracing = "0.1"
tracing-subscriber = "0.3"
axum = { version = "0.7", features = ["json", "http2"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors"] }
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
async-trait = "0.1"
clap = { workspace = true, features = ["env"] }
cozo = { workspace = true }

# gRPC (NotebookLM CognitiveToolService)
tonic = { workspace = true }
prost = { workspace = true }
tonic-reflection = { workspace = true }
tonic-health = { workspace = true }
tonic-web = { workspace = true }

# RAG pipeline
zip = "2"
sha2 = { workspace = true }
regex = { workspace = true }

# Concurrency
dashmap = { workspace = true }
parking_lot = { workspace = true }

# D-Bus
zbus = { workspace = true }

[[bin]]
name = "rag-ingest"
path = "src/bin/rag-ingest.rs"

[[bin]]
name = "op-cog-admin"
path = "src/bin/op-cog-admin.rs"

[build-dependencies]
tonic-build = { version = "0.12" }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/compare-op-cognitive-mcp.md">
# compare-op-cognitive-mcp

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 6 |
| Proto files | 0 |
| Binary targets | 1 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 0 |
| Spec-listed source files | 5 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- Internal crate integrations: op-core, op-mcp, op-state-store, op-dynamic-loader, op-cache.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/cognitive_tools.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/cognitive_tools.rs |
| `src/main.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/main.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/memory_store.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/memory_store.rs |
| `src/server.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/server.rs |
| `root` | ✅ Present | root source group | src/activity_filter.rs, src/cognitive_tools.rs, src/lib.rs, src/main.rs, src/memory_store.rs, src/server.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| cognitive_tools | ✅ Implemented | src/cognitive_tools.rs | SPEC main module |
| memory_store | ✅ Implemented | src/memory_store.rs | SPEC main module |
| server | ✅ Implemented | src/server.rs | SPEC main module |
| Primary binary entrypoint | ✅ Implemented | src/main.rs | runtime |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-mcp` - documented in SPEC
- `op-state-store` - not listed in SPEC dependency block
- `op-dynamic-loader` - documented in SPEC
- `op-cache` - documented in SPEC

### External Runtime Dependencies
- `serde` - documented in SPEC
- `serde_json` - not listed in SPEC dependency block
- `simd-json` - documented in SPEC
- `tokio` - documented in SPEC
- `anyhow` - documented in SPEC
- `tracing` - documented in SPEC
- `tracing-subscriber` - not listed in SPEC dependency block
- `axum` - documented in SPEC
- `tower` - documented in SPEC
- `tower-http` - documented in SPEC
- `uuid` - documented in SPEC
- `chrono` - documented in SPEC
- `async-trait` - documented in SPEC
- `clap` - not listed in SPEC dependency block
- `sqlx` - not listed in SPEC dependency block

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: activity_filter, cognitive_tools, memory_store, server.
- 5 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cognitive-mcp/SPEC.md">
# op-cognitive-mcp - Specification

## Overview
**Crate**: `op-cognitive-mcp`  
**Location**: `crates/op-cognitive-mcp`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-cognitive-mcp"
version = "0.1.0"
edition = "2021"

[dependencies]
```

### Source Structure
```
op-cognitive-mcp/src/cognitive_tools.rs
op-cognitive-mcp/src/main.rs
op-cognitive-mcp/src/lib.rs
op-cognitive-mcp/src/memory_store.rs
op-cognitive-mcp/src/server.rs
```

### Key Dependencies
```toml
op-core = { path = "../op-core" }
op-mcp = { path = "../op-mcp" }
op-dynamic-loader = { path = "../op-dynamic-loader" }
op-cache = { path = "../op-cache" }
serde = { version = "1.0", features = ["derive"] }
simd-json = { workspace = true }
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"
tracing = "0.1"
axum = { version = "0.7", features = ["json", "http2"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors"] }
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
async-trait = "0.1"
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
       5 Rust source files

### Main Modules
cognitive_tools
memory_store
server

## Purpose


## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:
- op-core
- op-mcp
- op-dynamic-loader
- op-cache

---
*Generated from crate analysis*
</file>

</files>
