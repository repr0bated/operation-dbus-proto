//! jacob-bd `nlm` CLI mapping for the `notebooklm` plugin.
//!
//! The executable lives at `/usr/local/bin/nlm` (override with `OP_NLM_BIN`).
//! `/usr/bin/nlm` is a different Node package and must never be invoked.

use anyhow::{anyhow, bail, Context};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub const DEFAULT_NLM_BIN: &str = "/usr/local/bin/nlm";
pub const NOTEBOOKLM_BIN_READY: &str = "/run/opdbus/runit-ready/notebooklm-mcp";
pub const NOTEBOOKLM_AUTH_READY: &str = "/run/opdbus/runit-ready/notebooklm-mcp-authenticated";
pub const DEFAULT_HOME: &str = "/home/jeremy";
pub const DEFAULT_DISPLAY: &str = ":20";
pub const DEFAULT_XAUTHORITY: &str = "/home/jeremy/.Xauthority";

const DEFAULT_CALL_TIMEOUT_SECS: u64 = 30;
const AUTH_TIMEOUT_SECS: u64 = 660;
const LONG_TIMEOUT_SECS: u64 = 120;

const STRIP_ENV: &[&str] = &[
    "NOTEBOOKLM_COOKIES",
    "NOTEBOOKLM_CSRF_TOKEN",
    "NOTEBOOKLM_SESSION_ID",
    "NOTEBOOKLM_COOKIE",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMarkerAction {
    SetReady,
    Clear,
    Leave,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NlmInvocation {
    pub argv: Vec<String>,
    pub interactive: bool,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotebooklmDispatch {
    LocalSelect { notebook_id: String },
    Cli(NlmInvocation),
}

pub fn nlm_bin() -> PathBuf {
    std::env::var("OP_NLM_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_NLM_BIN))
}

pub fn notebooklm_dispatch(
    method: &str,
    args: &Value,
    selected_notebook_id: Option<&str>,
) -> anyhow::Result<NotebooklmDispatch> {
    if method == "select_notebook" {
        let notebook_id = require_notebook_id(args, None)?;
        return Ok(NotebooklmDispatch::LocalSelect { notebook_id });
    }
    Ok(NotebooklmDispatch::Cli(nlm_argv(
        method,
        args,
        selected_notebook_id,
    )?))
}

pub fn nlm_argv(
    method: &str,
    args: &Value,
    selected_notebook_id: Option<&str>,
) -> anyhow::Result<NlmInvocation> {
    let argv = match method {
        // The pinned CLI has no JSON login/check endpoint. A real catalog
        // request is the authentication probe; its contents are not returned
        // from health methods (see run_notebooklm_method).
        "get_health" | "server_info" => vec_str(["notebook", "list", "--json"]),
        "setup_auth" => vec_str(["login"]),
        "reauth" => vec_str(["login", "--force"]),
        "refresh_auth" => vec_str(["auth", "refresh"]),
        "save_auth_tokens" => save_auth_tokens_argv(args)?,
        "notebook_list" => vec_str(["notebook", "list", "--json"]),
        "notebook_create" => {
            let title = require_str(args, &["title"])?;
            vec_owned(["notebook", "create", &title, "--json"])
        }
        "notebook_get" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            vec_owned(["notebook", "get", &id, "--json"])
        }
        "notebook_describe" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            vec_owned(["notebook", "describe", &id, "--json"])
        }
        "notebook_rename" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            let title = require_str(args, &["title", "new_title"])?;
            vec_owned(["notebook", "rename", &id, &title])
        }
        "notebook_delete" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            confirm_required(args, method)?;
            vec_owned(["notebook", "delete", &id, "--confirm", "--json"])
        }
        "notebook_query" => notebook_query_argv(args, selected_notebook_id, false)?,
        "notebook_query_start" | "notebook_query_status" => {
            bail!("{method} is MCP-only; the nlm CLI has no corresponding command")
        }
        "source_add" => source_add_argv(args, selected_notebook_id)?,
        "source_list_drive" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            let mut argv = vec_owned(["source", "list", &id, "--drive", "--json"]);
            if arg_bool(args, "skip_freshness") {
                argv.push("-S".into());
            }
            argv
        }
        "source_sync_drive" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            confirm_required(args, method)?;
            let mut argv = vec_owned(["source", "sync", &id, "--confirm"]);
            if let Some(ids) = csv_or_list(args, "source_ids") {
                argv.push("--source-ids".into());
                argv.push(ids);
            }
            argv
        }
        "source_delete" => source_delete_argv(args)?,
        "source_describe" => {
            let source_id = require_str(args, &["source_id", "id"])?;
            vec_owned(["source", "describe", &source_id, "--json"])
        }
        "source_get_content" => {
            let source_id = require_str(args, &["source_id", "id"])?;
            let mut argv = vec_owned(["source", "content", &source_id, "--json"]);
            if let Some(path) = arg_str(args, &["output_path", "output"]) {
                argv.push("-o".into());
                argv.push(path);
            }
            argv
        }
        "source_rename" => {
            let source_id = require_str(args, &["source_id", "id"])?;
            let title = require_str(args, &["new_title", "title"])?;
            let notebook = require_notebook_id(args, selected_notebook_id)?;
            vec_owned([
                "source",
                "rename",
                &source_id,
                &title,
                "--notebook",
                &notebook,
            ])
        }
        "chat_configure" => chat_configure_argv(args, selected_notebook_id)?,
        "chat_list" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            vec_owned(["chats", "list", &id, "--json"])
        }
        "chat_get" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            let mut argv = vec_owned(["chats", "get", &id]);
            if let Some(chat_id) = arg_str(args, &["chat_id"]) {
                argv.push(chat_id);
            }
            argv.push("--json".into());
            argv
        }
        "chat_export" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            let mut argv = vec_owned(["chats", "export", &id]);
            if let Some(chat_id) = arg_str(args, &["chat_id", "id"]) {
                argv.push("--conversation-id".into());
                argv.push(chat_id);
            }
            if let Some(format) = arg_str(args, &["format"]) {
                argv.extend(["--format".into(), format]);
            }
            if let Some(path) = arg_str(args, &["output_path", "output"]) {
                argv.extend(["--output".into(), path]);
            }
            argv
        }
        "studio_create" => studio_create_argv(args, selected_notebook_id)?,
        "studio_status" => studio_status_argv(args, selected_notebook_id)?,
        "studio_delete" => {
            let notebook = require_notebook_id(args, selected_notebook_id)?;
            let artifact = require_str(args, &["artifact_id"])?;
            confirm_required(args, method)?;
            vec_owned(["studio", "delete", &notebook, &artifact, "--confirm"])
        }
        "studio_revise" => {
            let artifact = require_str(args, &["artifact_id"])?;
            let instruction = require_str(args, &["instruction", "slide", "prompt"])?;
            confirm_required(args, method)?;
            vec_owned([
                "slides",
                "revise",
                &artifact,
                "--slide",
                &instruction,
                "--confirm",
            ])
        }
        "download_artifact" => download_artifact_argv(args, selected_notebook_id)?,
        "download_all_artifacts" => download_all_argv(args, selected_notebook_id)?,
        "export_artifact" => export_artifact_argv(args, selected_notebook_id)?,
        "research_start" => research_start_argv(args, selected_notebook_id)?,
        "research_status" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            let mut argv = vec_owned(["research", "status", &id]);
            if let Some(task) = arg_str(args, &["task_id"]) {
                argv.push("--task-id".into());
                argv.push(task);
            }
            if arg_bool(args, "full") {
                argv.push("--full".into());
            }
            argv
        }
        "research_import" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            let task = require_str(args, &["task_id"])?;
            let mut argv = vec_owned(["research", "import", &id, &task]);
            if let Some(indices) = csv_or_list(args, "indices") {
                argv.push("--indices".into());
                argv.push(indices);
            }
            if arg_bool(args, "cited_only") {
                argv.push("--cited-only".into());
            }
            if let Some(timeout) = arg_u64(args, "timeout") {
                argv.push("--timeout".into());
                argv.push(timeout.to_string());
            }
            argv
        }
        "note" => note_argv(args, selected_notebook_id)?,
        "label" => label_argv(args, selected_notebook_id)?,
        "notebook_share_status" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            vec_owned(["share", "status", &id, "--json"])
        }
        "notebook_share_public" => share_public_argv(args, selected_notebook_id)?,
        "notebook_share_invite" => {
            let id = require_notebook_id(args, selected_notebook_id)?;
            let email = require_str(args, &["email", "recipient"])?;
            let mut argv = vec_owned(["share", "invite", &id, &email]);
            if let Some(role) = arg_str(args, &["role"]) {
                argv.push("--role".into());
                argv.push(role);
            }
            argv
        }
        "notebook_share_batch" => share_batch_argv(args, selected_notebook_id)?,
        "batch" => batch_argv(args)?,
        "cross_notebook_query" => cross_query_argv(args)?,
        "pipeline" => pipeline_argv(args, selected_notebook_id)?,
        "tag" => tag_argv(args, selected_notebook_id)?,
        other => bail!("unknown notebooklm method '{other}' (no PleasePrompto aliases)"),
    };

    Ok(NlmInvocation {
        interactive: matches!(method, "setup_auth" | "reauth"),
        timeout_secs: timeout_for(method, args),
        argv,
    })
}

pub fn redact_nlm_text(input: &str) -> String {
    let mut out = input.to_string();
    for key in [
        "NOTEBOOKLM_COOKIES",
        "NOTEBOOKLM_CSRF_TOKEN",
        "NOTEBOOKLM_SESSION_ID",
        "NOTEBOOKLM_COOKIE",
        "SID",
        "HSID",
        "SSID",
        "APISID",
        "SAPISID",
        "__Secure-1PSID",
        "__Secure-3PSID",
        "__Secure-1PSIDTS",
        "__Secure-3PSIDTS",
        "NID",
    ] {
        out = redact_assignment(&out, key);
    }
    out
}

pub fn auth_status_from_value(value: &Value) -> Option<String> {
    find_string_field(value, "auth_status")
}

pub fn ready_marker_action(status: &str) -> AuthMarkerAction {
    match status {
        "configured" => AuthMarkerAction::SetReady,
        "unverified" => AuthMarkerAction::Leave,
        _ => AuthMarkerAction::Clear,
    }
}

pub fn apply_ready_marker(path: &str, action: AuthMarkerAction) -> std::io::Result<()> {
    match action {
        AuthMarkerAction::SetReady => std::fs::write(path, b"ready\n"),
        AuthMarkerAction::Clear => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
        AuthMarkerAction::Leave => Ok(()),
    }
}

pub fn touch_bin_ready_marker(bin: &Path) -> std::io::Result<()> {
    if bin.is_file() {
        if let Some(parent) = Path::new(NOTEBOOKLM_BIN_READY).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(NOTEBOOKLM_BIN_READY, b"ready\n")
    } else {
        match std::fs::remove_file(NOTEBOOKLM_BIN_READY) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

pub async fn run_nlm(invocation: &NlmInvocation) -> anyhow::Result<Value> {
    let bin = nlm_bin();
    if !bin.is_file() {
        bail!(
            "nlm binary missing at {} (install jacob-bd notebooklm-mcp-cli; do not use /usr/bin/nlm)",
            bin.display()
        );
    }

    let mut command = Command::new(&bin);
    command.kill_on_drop(true);
    command.args(&invocation.argv);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.stdin(Stdio::null());
    // HOME alone does not change the OS identity. In particular, never launch
    // Chrome as the root bridge or leave root-owned authentication profiles.
    let user = nix::unistd::User::from_name("jeremy")?
        .ok_or_else(|| anyhow!("NotebookLM runtime account is missing"))?;
    let uid = nix::unistd::geteuid();
    if uid.is_root() {
        let gid = user.gid;
        let user_id = user.uid;
        // Only async-signal-safe libc operations in the forked child. Clear
        // supplementary groups before dropping gid/uid.
        unsafe {
            command.pre_exec(move || {
                nix::unistd::setgroups(&[])?;
                nix::unistd::setgid(gid)?;
                nix::unistd::setuid(user_id)?;
                Ok(())
            });
        }
    } else if uid != user.uid {
        bail!("NotebookLM must run as its configured account");
    }
    command.env("HOME", DEFAULT_HOME);
    command.env("USER", "jeremy");
    command.env("PATH", "/usr/local/bin:/usr/bin:/bin");
    for key in STRIP_ENV {
        command.env_remove(key);
    }
    if invocation.interactive {
        command.env("DISPLAY", DEFAULT_DISPLAY);
        command.env("XAUTHORITY", DEFAULT_XAUTHORITY);
    }

    let output = tokio::time::timeout(
        Duration::from_secs(invocation.timeout_secs),
        command.output(),
    )
    .await
    .map_err(|_| anyhow!("nlm timed out after {}s", invocation.timeout_secs))?
    .with_context(|| format!("spawn {}", bin.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = redact_nlm_text(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        bail!("nlm exited {code}: {}", stderr.trim());
    }

    if invocation.argv.iter().any(|arg| arg == "--json") {
        parse_structured_stdout(&stdout)
    } else {
        // A receipt reports process completion, not fabricated domain fields.
        // Rich output is diagnostic text, never parsed into provider objects.
        Ok(serde_json::json!({
            "status": "command_completed",
            "output_format": "text",
            "message": redact_nlm_text(&stdout),
            "diagnostic": stderr,
        }))
    }
}

pub async fn run_notebooklm_method(
    method: &str,
    invocation: &NlmInvocation,
) -> anyhow::Result<Value> {
    let health = matches!(method, "get_health" | "server_info");
    let auth = matches!(
        method,
        "setup_auth" | "reauth" | "refresh_auth" | "save_auth_tokens"
    );
    if !health && !auth {
        return run_nlm(invocation).await;
    }
    // Invalidate readiness before any attempt: cancellation, timeout, malformed
    // output or failed login must not leave a previous authenticated marker.
    apply_ready_marker(NOTEBOOKLM_AUTH_READY, AuthMarkerAction::Clear)?;
    if auth {
        run_nlm(invocation).await?;
    }
    let probe = nlm_argv("get_health", &serde_json::json!({}), None)?;
    match run_nlm(&probe).await {
        Ok(Value::Array(notebooks)) => Ok(serde_json::json!({
            "provider": "jacob-bd/notebooklm-mcp-cli",
            "auth_status": "configured",
            "authenticated": true,
            "notebook_count": notebooks.len(),
        })),
        Ok(_) => bail!("NotebookLM authentication probe returned an unexpected catalog shape"),
        Err(error) if health => Ok(serde_json::json!({
            "provider": "jacob-bd/notebooklm-mcp-cli",
            "auth_status": "unverified",
            "authenticated": false,
            "diagnostic": redact_nlm_text(&error.to_string()),
        })),
        Err(error) => {
            Err(error.context("NotebookLM login completed but Google access was not verified"))
        }
    }
}

fn parse_structured_stdout(stdout: &str) -> anyhow::Result<Value> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        bail!("nlm produced no structured output (need --json or --ai)");
    }
    serde_json::from_str(trimmed)
        .map_err(|_| anyhow!("nlm produced invalid JSON stdout; refusing to scrape Rich tables"))
}

fn timeout_for(method: &str, args: &Value) -> u64 {
    if let Some(timeout) = arg_u64(args, "timeout") {
        return timeout.clamp(1, 900);
    }
    match method {
        "setup_auth" | "reauth" => AUTH_TIMEOUT_SECS,
        "research_start"
        | "research_status"
        | "research_import"
        | "studio_create"
        | "notebook_query"
        | "notebook_query_start" => LONG_TIMEOUT_SECS,
        _ => DEFAULT_CALL_TIMEOUT_SECS,
    }
}

fn notebook_query_argv(
    args: &Value,
    selected: Option<&str>,
    start: bool,
) -> anyhow::Result<Vec<String>> {
    let id = require_notebook_id(args, selected)?;
    let question = require_str(args, &["question", "query"])?;
    let mut argv = vec_owned(["notebook", "query", &id, &question, "--json"]);
    if start {
        argv.push("--wait".into());
        argv.push("0".into());
    }
    Ok(argv)
}

fn source_add_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let id = require_notebook_id(args, selected)?;
    let source_type = arg_str(args, &["source_type", "type"]).unwrap_or_else(|| "url".into());
    let mut argv = vec_owned(["source", "add", &id]);
    match source_type.as_str() {
        "url" => {
            if let Some(urls) = args.get("urls").and_then(Value::as_array) {
                for url in urls.iter().filter_map(Value::as_str) {
                    argv.push("--url".into());
                    argv.push(url.to_string());
                }
            } else {
                argv.push("--url".into());
                argv.push(require_str(args, &["url"])?);
            }
        }
        "text" => {
            argv.push("--text".into());
            argv.push(require_str(args, &["text", "content"])?);
            if let Some(title) = arg_str(args, &["title"]) {
                argv.push("--title".into());
                argv.push(title);
            }
        }
        "file" => {
            argv.push("--file".into());
            argv.push(require_str(args, &["file_path", "path", "file"])?);
        }
        "drive" => {
            argv.push("--drive".into());
            argv.push(require_str(args, &["document_id", "drive", "id"])?);
            if let Some(kind) = arg_str(args, &["doc_type", "type"]) {
                if kind != "drive" {
                    argv.push("--type".into());
                    argv.push(kind);
                }
            }
        }
        other => bail!("unsupported source_type '{other}'"),
    }
    argv.push("--json".into());
    Ok(argv)
}

fn source_delete_argv(args: &Value) -> anyhow::Result<Vec<String>> {
    confirm_required(args, "source_delete")?;
    if let Some(ids) = args.get("source_ids").and_then(Value::as_array) {
        let mut argv = vec_str(["source", "delete"]);
        for id in ids.iter().filter_map(Value::as_str) {
            argv.push(id.to_string());
        }
        argv.push("--confirm".into());
        argv.push("--json".into());
        return Ok(argv);
    }
    let source_id = require_str(args, &["source_id", "id"])?;
    Ok(vec_owned([
        "source",
        "delete",
        &source_id,
        "--confirm",
        "--json",
    ]))
}

fn chat_configure_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let id = require_notebook_id(args, selected)?;
    let mut argv = vec_owned(["chat", "configure", &id]);
    if let Some(goal) = arg_str(args, &["goal"]) {
        argv.push("--goal".into());
        argv.push(goal);
    }
    if let Some(prompt) = arg_str(args, &["prompt", "custom_prompt"]) {
        argv.push("--prompt".into());
        argv.push(prompt);
    }
    if let Some(length) = arg_str(args, &["response_length"]) {
        argv.push("--response-length".into());
        argv.push(length);
    }
    Ok(argv)
}

fn studio_create_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let id = require_notebook_id(args, selected)?;
    let artifact = require_str(args, &["artifact_type", "type"])?;
    confirm_required(args, "studio_create")?;
    let cli_noun = studio_noun(&artifact)?;
    let mut argv = vec![
        cli_noun,
        "create".into(),
        id,
        "--confirm".into(),
        "--json".into(),
    ];
    if let Some(format) = arg_str(args, &["format"]) {
        argv.push("--format".into());
        argv.push(format);
    }
    if let Some(length) = arg_str(args, &["length"]) {
        argv.push("--length".into());
        argv.push(length);
    }
    if let Some(focus) = arg_str(args, &["focus", "focus_prompt"]) {
        argv.push("--focus".into());
        argv.push(focus);
    }
    if let Some(prompt) = arg_str(args, &["prompt", "custom_prompt"]) {
        argv.push("--prompt".into());
        argv.push(prompt);
    }
    if artifact == "data_table" || artifact == "data-table" {
        if let Some(description) = arg_str(args, &["description"]) {
            argv.insert(3, description);
        }
    }
    Ok(argv)
}

fn studio_noun(artifact: &str) -> anyhow::Result<String> {
    Ok(match artifact {
        "audio" => "audio",
        "video" => "video",
        "report" => "report",
        "quiz" => "quiz",
        "flashcards" => "flashcards",
        "infographic" => "infographic",
        "mindmap" | "mind_map" => "mindmap",
        "slides" | "slide_deck" | "slide-deck" => "slides",
        "data_table" | "data-table" => "data-table",
        other => bail!("unsupported studio artifact_type '{other}'"),
    }
    .into())
}

fn studio_status_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let id = require_notebook_id(args, selected)?;
    let mut argv = vec_owned(["studio", "status", &id, "--json"]);
    if arg_bool(args, "full") || arg_bool(args, "include_details") {
        argv.push("--full".into());
    }
    if let Some(artifact) = arg_str(args, &["artifact_id"]) {
        argv.push("--artifact-id".into());
        argv.push(artifact);
    }
    if let Some(action) = arg_str(args, &["action"]) {
        if action == "rename" {
            let artifact = require_str(args, &["artifact_id"])?;
            let title = require_str(args, &["new_title", "title"])?;
            return Ok(vec_owned(["studio", "rename", &artifact, &title]));
        }
    }
    Ok(argv)
}

fn download_artifact_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let id = require_notebook_id(args, selected)?;
    let artifact = require_str(args, &["artifact_type", "type"])?;
    let output = require_str(args, &["output_path", "output"])?;
    let noun = match artifact.as_str() {
        "slide_deck" | "slides" => "slide-deck".into(),
        other => other.replace('_', "-"),
    };
    let mut argv = vec_owned(["download", &noun, &id, "--output", &output, "--json"]);
    if let Some(format) = arg_str(args, &["format"]) {
        argv.push("--format".into());
        argv.push(format);
    }
    Ok(argv)
}

fn download_all_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let mut argv = vec_str(["download", "all"]);
    if arg_bool(args, "all_notebooks") {
        argv.push("--all-notebooks".into());
    } else {
        argv.push(require_notebook_id(args, selected)?);
    }
    if let Some(dir) = arg_str(args, &["output_dir", "directory", "output"]) {
        argv.push("-d".into());
        argv.push(dir);
    }
    if arg_bool(args, "skip_existing") {
        argv.push("--skip-existing".into());
    }
    argv.push("--json".into());
    Ok(argv)
}

fn export_artifact_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let id = require_notebook_id(args, selected)?;
    let artifact = require_str(args, &["artifact_id"])?;
    let export_type = require_str(args, &["export_type", "type"])?;
    let mut argv = vec_owned([
        "export",
        "artifact",
        &id,
        &artifact,
        "--type",
        &export_type,
        "--json",
    ]);
    if let Some(title) = arg_str(args, &["title"]) {
        argv.push("--title".into());
        argv.push(title);
    }
    Ok(argv)
}

fn research_start_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let query = require_str(args, &["query", "question"])?;
    let mut argv = vec_owned(["research", "start", &query]);
    if let Some(id) = arg_str(args, &["notebook_id", "id"]).or_else(|| selected.map(str::to_string))
    {
        argv.push("--notebook-id".into());
        argv.push(id);
    }
    if let Some(title) = arg_str(args, &["title"]) {
        argv.push("--title".into());
        argv.push(title);
    }
    if let Some(mode) = arg_str(args, &["mode"]) {
        argv.push("--mode".into());
        argv.push(mode);
    }
    if let Some(source) = arg_str(args, &["source"]) {
        argv.push("--source".into());
        argv.push(source);
    }
    if arg_bool(args, "auto_import") {
        argv.push("--auto-import".into());
    }
    Ok(argv)
}

fn note_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let action = require_str(args, &["action"])?;
    let id = require_notebook_id(args, selected)?;
    let mut argv = vec_owned(["note", &action, &id]);
    match action.as_str() {
        "create" => {
            argv.push("--content".into());
            argv.push(require_str(args, &["content"])?);
            if let Some(title) = arg_str(args, &["title"]) {
                argv.push("--title".into());
                argv.push(title);
            }
        }
        "list" => {}
        "update" => {
            argv.push(require_str(args, &["note_id"])?);
            argv.push("--content".into());
            argv.push(require_str(args, &["content"])?);
            if let Some(title) = arg_str(args, &["title"]) {
                argv.push("--title".into());
                argv.push(title);
            }
        }
        "delete" => {
            confirm_required(args, "note")?;
            argv.push(require_str(args, &["note_id"])?);
            argv.push("--confirm".into());
        }
        other => bail!("unsupported note action '{other}'"),
    }
    if action == "list" {
        argv.push("--json".into());
    }
    Ok(argv)
}

fn label_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let action = require_str(args, &["action"])?;
    let mut argv = vec_str(["label", &action]);
    match action.as_str() {
        "list" => {
            argv.push(require_notebook_id(args, selected)?);
        }
        "auto" | "reorganize" => {
            argv.push(require_notebook_id(args, selected)?);
            if arg_bool(args, "confirm") {
                argv.push("--confirm".into());
            }
            if arg_bool(args, "unlabeled_only") {
                argv.push("--unlabeled-only".into());
            }
        }
        "create" => {
            argv.push(require_notebook_id(args, selected)?);
            argv.push(require_str(args, &["name", "title"])?);
        }
        "rename" => {
            argv.push(require_notebook_id(args, selected)?);
            argv.push(require_str(args, &["label_id", "id"])?);
            argv.push(require_str(args, &["new_name", "name", "title"])?);
        }
        "set_emoji" | "emoji" => {
            argv[1] = "emoji".into();
            argv.push(require_notebook_id(args, selected)?);
            argv.push(require_str(args, &["label_id", "id"])?);
            argv.push(require_str(args, &["emoji"])?);
        }
        "move_source" | "move" => {
            argv[1] = "move".into();
            argv.push(require_notebook_id(args, selected)?);
            argv.push(require_str(args, &["source_id"])?);
            argv.push(require_str(args, &["label_id", "id"])?);
        }
        "delete" => {
            confirm_required(args, "label")?;
            argv.push(require_notebook_id(args, selected)?);
            argv.push(require_str(args, &["label_id", "id"])?);
            argv.push("--confirm".into());
        }
        other => bail!("unsupported label action '{other}'"),
    }
    argv.push("--json".into());
    Ok(argv)
}

fn share_public_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let id = require_notebook_id(args, selected)?;
    let argv = if arg_bool(args, "off")
        || arg_bool(args, "disable")
        || arg_str(args, &["action"]).as_deref() == Some("disable")
    {
        vec_owned(["share", "private", &id])
    } else {
        vec_owned(["share", "public", &id])
    };
    Ok(argv)
}

fn share_batch_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    confirm_required(args, "notebook_share_batch")?;
    let id = require_notebook_id(args, selected)?;
    let emails = arg_str(args, &["emails"])
        .or_else(|| {
            args.get("recipients")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
        })
        .ok_or_else(|| anyhow!("emails/recipients is required"))?;
    let argv = vec_owned(["share", "batch", &id, &emails]);
    Ok(argv)
}

fn batch_argv(args: &Value) -> anyhow::Result<Vec<String>> {
    let action = require_str(args, &["action"])?;
    let cli_action = action.replace('_', "-");
    let mut argv = vec_owned(["batch", &cli_action]);
    match action.as_str() {
        "query" => argv.push(require_str(args, &["query", "question"])?),
        "add_source" | "add-source" => argv.push(require_str(args, &["source_url", "url"])?),
        "create" => argv.push(require_str(args, &["titles", "title"])?),
        "studio" => argv.push(require_str(args, &["artifact_type", "type"])?),
        "delete" => confirm_required(args, "batch")?,
        _ => {}
    }
    if let Some(notebooks) = arg_str(args, &["notebook_names", "notebooks"]) {
        argv.push("--notebooks".into());
        argv.push(notebooks);
    }
    if let Some(tags) = arg_str(args, &["tags"]) {
        argv.push("--tags".into());
        argv.push(tags);
    }
    if arg_bool(args, "all") {
        argv.push("--all".into());
    }
    if arg_bool(args, "confirm") {
        argv.push("--confirm".into());
    }
    Ok(argv)
}

fn cross_query_argv(args: &Value) -> anyhow::Result<Vec<String>> {
    let query = require_str(args, &["query", "question"])?;
    let mut argv = vec_owned(["cross", "query", &query]);
    if let Some(notebooks) = arg_str(args, &["notebook_names", "notebooks"]) {
        argv.push("--notebooks".into());
        argv.push(notebooks);
    }
    if let Some(tags) = arg_str(args, &["tags"]) {
        argv.push("--tags".into());
        argv.push(tags);
    }
    if arg_bool(args, "all") {
        argv.push("--all".into());
    }
    Ok(argv)
}

fn pipeline_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let action = require_str(args, &["action"])?;
    match action.as_str() {
        "list" => Ok(vec_str(["pipeline", "list"])),
        "run" => {
            let name = require_str(args, &["pipeline_name", "name"])?;
            let id = arg_str(args, &["notebook_id"])
                .or_else(|| selected.map(str::to_string))
                .ok_or_else(|| anyhow!("notebook_id is required for pipeline run"))?;
            let mut argv = vec_owned(["pipeline", "run", &name, "--notebook", &id]);
            if let Some(url) = arg_str(args, &["input_url"]) {
                argv.push("--input-url".into());
                argv.push(url);
            }
            Ok(argv)
        }
        "create" => {
            let name = require_str(args, &["pipeline_name", "name"])?;
            let file = require_str(args, &["file", "file_path"])?;
            Ok(vec_owned(["pipeline", "create", &name, "--file", &file]))
        }
        other => bail!("unsupported pipeline action '{other}'"),
    }
}

fn tag_argv(args: &Value, selected: Option<&str>) -> anyhow::Result<Vec<String>> {
    let action = require_str(args, &["action"])?;
    match action.as_str() {
        "list" => Ok(vec_str(["tag", "list"])),
        "select" => {
            let query = require_str(args, &["query"])?;
            Ok(vec_owned(["tag", "select", &query]))
        }
        "add" | "remove" => {
            let id = require_notebook_id(args, selected)?;
            let tags = require_str(args, &["tags"])?;
            let mut argv = vec_owned(["tag", &action, &id, "--tags", &tags]);
            if let Some(title) = arg_str(args, &["title"]) {
                argv.push("--title".into());
                argv.push(title);
            }
            Ok(argv)
        }
        other => bail!("unsupported tag action '{other}'"),
    }
}

fn save_auth_tokens_argv(args: &Value) -> anyhow::Result<Vec<String>> {
    if arg_str(args, &["cookies", "cookie"]).is_some() {
        bail!("save_auth_tokens refuses inline cookies; pass file_path and never set NOTEBOOKLM_COOKIES");
    }
    let path = require_str(args, &["file_path", "path"])?;
    Ok(vec_owned(["login", "--manual", "--file", &path]))
}

fn require_notebook_id(args: &Value, selected: Option<&str>) -> anyhow::Result<String> {
    arg_str(args, &["notebook_id", "id"])
        .or_else(|| selected.map(str::to_string))
        .ok_or_else(|| anyhow!("notebook_id is required (call select_notebook or pass id)"))
}

fn require_str(args: &Value, keys: &[&str]) -> anyhow::Result<String> {
    arg_str(args, keys).ok_or_else(|| anyhow!("missing required argument {}", keys.join("/")))
}

fn arg_str(args: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        match args.get(*key) {
            Some(Value::String(s)) if !s.is_empty() => return Some(s.clone()),
            Some(Value::Number(n)) => return Some(n.to_string()),
            _ => {}
        }
    }
    None
}

fn arg_bool(args: &Value, key: &str) -> bool {
    match args.get(key) {
        Some(Value::Bool(v)) => *v,
        Some(Value::String(s)) => s == "true" || s == "1",
        _ => false,
    }
}

fn arg_u64(args: &Value, key: &str) -> Option<u64> {
    match args.get(key) {
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn csv_or_list(args: &Value, key: &str) -> Option<String> {
    match args.get(key) {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Array(items)) => {
            let joined = items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        }
        _ => None,
    }
}

fn confirm_required(args: &Value, method: &str) -> anyhow::Result<()> {
    if arg_bool(args, "confirm") {
        Ok(())
    } else {
        bail!("{method} requires confirm=true")
    }
}

fn find_string_field(value: &Value, field: &str) -> Option<String> {
    match value {
        Value::Object(object) => object
            .get(field)
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                object
                    .values()
                    .find_map(|nested| find_string_field(nested, field))
            }),
        Value::Array(values) => values
            .iter()
            .find_map(|nested| find_string_field(nested, field)),
        _ => None,
    }
}

fn redact_assignment(input: &str, key: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    let needle = format!("{key}=");
    while let Some(idx) = rest.find(&needle) {
        out.push_str(&rest[..idx]);
        out.push_str(&needle);
        out.push_str("[redacted]");
        rest = &rest[idx + needle.len()..];
        let end = rest
            .find(|c: char| c == ';' || c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(rest.len());
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn vec_str<const N: usize>(parts: [&str; N]) -> Vec<String> {
    parts.into_iter().map(str::to_string).collect()
}

fn vec_owned<const N: usize>(parts: [&str; N]) -> Vec<String> {
    parts.into_iter().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn argv(method: &str, args: Value, selected: Option<&str>) -> Vec<String> {
        match notebooklm_dispatch(method, &args, selected).unwrap() {
            NotebooklmDispatch::Cli(inv) => inv.argv,
            NotebooklmDispatch::LocalSelect { notebook_id } => {
                panic!("expected CLI, got local select {notebook_id}")
            }
        }
    }

    #[test]
    fn every_schema_method_has_an_argv_map() {
        let selected = Some("nb-1");
        let empty = json!({});
        let with_id = json!({"id": "nb-1", "notebook_id": "nb-1", "confirm": true});
        let cases: &[(&str, Value)] = &[
            ("get_health", empty.clone()),
            ("server_info", empty.clone()),
            ("setup_auth", empty.clone()),
            ("reauth", empty.clone()),
            ("refresh_auth", empty.clone()),
            ("save_auth_tokens", json!({"file_path": "/tmp/cookies.txt"})),
            ("notebook_list", empty.clone()),
            ("notebook_create", json!({"title": "Research"})),
            ("notebook_get", with_id.clone()),
            ("notebook_describe", with_id.clone()),
            ("notebook_rename", json!({"id": "nb-1", "title": "New"})),
            ("notebook_delete", json!({"id": "nb-1", "confirm": true})),
            (
                "notebook_query",
                json!({"notebook_id": "nb-1", "question": "What?"}),
            ),
            (
                "source_add",
                json!({"notebook_id": "nb-1", "source_type": "url", "url": "https://ex"}),
            ),
            ("source_list_drive", with_id.clone()),
            (
                "source_sync_drive",
                json!({"notebook_id": "nb-1", "confirm": true}),
            ),
            (
                "source_delete",
                json!({"source_id": "src-1", "confirm": true}),
            ),
            ("source_describe", json!({"source_id": "src-1"})),
            ("source_get_content", json!({"source_id": "src-1"})),
            (
                "source_rename",
                json!({"source_id": "src-1", "new_title": "T", "notebook_id": "nb-1"}),
            ),
            (
                "chat_configure",
                json!({"notebook_id": "nb-1", "goal": "default"}),
            ),
            ("chat_list", with_id.clone()),
            ("chat_get", json!({"notebook_id": "nb-1", "chat_id": "c1"})),
            (
                "chat_export",
                json!({"notebook_id": "nb-1", "chat_id": "c1"}),
            ),
            (
                "studio_create",
                json!({"notebook_id": "nb-1", "artifact_type": "audio", "confirm": true}),
            ),
            ("studio_status", with_id.clone()),
            (
                "studio_delete",
                json!({"notebook_id": "nb-1", "artifact_id": "a1", "confirm": true}),
            ),
            (
                "studio_revise",
                json!({"artifact_id": "a1", "instruction": "fix", "confirm": true}),
            ),
            (
                "download_artifact",
                json!({"notebook_id": "nb-1", "artifact_type": "audio", "output_path": "x.mp3"}),
            ),
            (
                "download_all_artifacts",
                json!({"notebook_id": "nb-1", "output_dir": "./out"}),
            ),
            (
                "export_artifact",
                json!({"notebook_id": "nb-1", "artifact_id": "a1", "export_type": "docs"}),
            ),
            (
                "research_start",
                json!({"query": "q", "notebook_id": "nb-1"}),
            ),
            ("research_status", with_id.clone()),
            (
                "research_import",
                json!({"notebook_id": "nb-1", "task_id": "t1"}),
            ),
            ("note", json!({"action": "list", "notebook_id": "nb-1"})),
            ("label", json!({"action": "list", "notebook_id": "nb-1"})),
            ("notebook_share_status", with_id.clone()),
            ("notebook_share_public", with_id.clone()),
            (
                "notebook_share_invite",
                json!({"notebook_id": "nb-1", "email": "a@b.c"}),
            ),
            (
                "notebook_share_batch",
                json!({"notebook_id": "nb-1", "confirm": true, "emails": "a@b.c"}),
            ),
            (
                "batch",
                json!({"action": "query", "query": "q", "all": true}),
            ),
            ("cross_notebook_query", json!({"query": "q", "all": true})),
            ("pipeline", json!({"action": "list"})),
            ("tag", json!({"action": "list"})),
            ("select_notebook", json!({"id": "nb-1"})),
        ];
        for (method, args) in cases {
            notebooklm_dispatch(method, args, selected)
                .unwrap_or_else(|e| panic!("{method} must map: {e}"));
        }
    }

    #[test]
    fn pleasepromto_names_are_rejected() {
        for method in [
            "list_notebooks",
            "query_notebook",
            "get_notebook",
            "search_notebooks",
            "list_sessions",
            "add_source_url",
            "create_audio",
        ] {
            let err = notebooklm_dispatch(method, &json!({}), None).unwrap_err();
            assert!(
                err.to_string().contains("no PleasePrompto aliases"),
                "{method}: {err}"
            );
        }
    }

    #[test]
    fn mcp_only_query_lifecycle_never_maps_to_cli() {
        for method in ["notebook_query_start", "notebook_query_status"] {
            let err = notebooklm_dispatch(method, &json!({}), Some("nb-1")).unwrap_err();
            assert!(err.to_string().contains("MCP-only"), "{method}: {err}");
        }
    }

    #[test]
    fn selected_notebook_fills_omitted_id() {
        let argv = argv(
            "notebook_get",
            json!({}),
            Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
        );
        assert_eq!(
            argv,
            vec![
                "notebook",
                "get",
                "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
                "--json"
            ]
        );
    }

    #[test]
    fn get_health_and_server_info_probe_real_catalog() {
        assert_eq!(
            argv("get_health", json!({}), None),
            argv("server_info", json!({}), None)
        );
        assert_eq!(
            argv("get_health", json!({}), None),
            vec!["notebook", "list", "--json"]
        );
    }

    #[test]
    fn setup_auth_is_interactive_login_without_cookie_env_shape() {
        let inv = match notebooklm_dispatch("setup_auth", &json!({}), None).unwrap() {
            NotebooklmDispatch::Cli(inv) => inv,
            NotebooklmDispatch::LocalSelect { .. } => panic!("setup_auth is CLI"),
        };
        assert_eq!(inv.argv, vec!["login"]);
        assert!(inv.interactive);
        assert_eq!(inv.timeout_secs, 660);
        assert!(!inv.argv.iter().any(|a| a.contains("NOTEBOOKLM_COOKIES")));
    }

    #[test]
    fn default_bin_is_usr_local_not_usr_bin() {
        std::env::remove_var("OP_NLM_BIN");
        assert_eq!(nlm_bin().as_os_str(), "/usr/local/bin/nlm");
    }

    #[test]
    fn cookie_strings_are_redacted_from_stderr() {
        let raw = "failed SID=secretvalue; HSID=other NOTEBOOKLM_COOKIES=abc123 leftover";
        let redacted = redact_nlm_text(raw);
        assert!(!redacted.contains("secretvalue"));
        assert!(!redacted.contains("other"));
        assert!(!redacted.contains("abc123"));
        assert!(redacted.contains("SID=[redacted]"));
        assert!(redacted.contains("NOTEBOOKLM_COOKIES=[redacted]"));
        assert!(redacted.contains("leftover"));
    }

    #[test]
    fn ready_marker_transitions() {
        assert_eq!(
            ready_marker_action("configured"),
            AuthMarkerAction::SetReady
        );
        assert_eq!(ready_marker_action("stale"), AuthMarkerAction::Clear);
        assert_eq!(
            ready_marker_action("not_configured"),
            AuthMarkerAction::Clear
        );
        assert_eq!(ready_marker_action("unverified"), AuthMarkerAction::Leave);
        assert_eq!(ready_marker_action("error"), AuthMarkerAction::Clear);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("marker");
        std::fs::write(&path, b"ready\n").unwrap();
        apply_ready_marker(path.to_str().unwrap(), AuthMarkerAction::Leave).unwrap();
        assert!(path.is_file());
        apply_ready_marker(path.to_str().unwrap(), AuthMarkerAction::Clear).unwrap();
        assert!(!path.is_file());
        apply_ready_marker(path.to_str().unwrap(), AuthMarkerAction::SetReady).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn auth_status_is_read_from_nested_json() {
        let value = json!({"result": {"auth_status": "configured"}});
        assert_eq!(
            auth_status_from_value(&value).as_deref(),
            Some("configured")
        );
    }

    #[test]
    fn save_auth_tokens_rejects_inline_cookies() {
        let err = notebooklm_dispatch("save_auth_tokens", &json!({"cookies": "SID=secret"}), None)
            .unwrap_err();
        assert!(err.to_string().contains("NOTEBOOKLM_COOKIES"));
    }

    #[test]
    fn source_add_url_and_drive_flags() {
        assert_eq!(
            argv(
                "source_add",
                json!({"notebook_id": "nb", "source_type": "url", "url": "https://ex"}),
                None
            ),
            vec!["source", "add", "nb", "--url", "https://ex", "--json"]
        );
        assert_eq!(
            argv(
                "source_add",
                json!({"notebook_id": "nb", "source_type": "drive", "document_id": "doc"}),
                None
            ),
            vec!["source", "add", "nb", "--drive", "doc", "--json"]
        );
    }

    #[test]
    fn structured_stdout_rejects_rich_tables() {
        let err = parse_structured_stdout("╭──────────╮\n│ notebooks │\n╰──────────╯").unwrap_err();
        assert!(err.to_string().contains("Rich"));
    }
}
