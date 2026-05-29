//! Codex CLI backend for the local OpenAI-compatible proxy.
//!
//! This keeps ChatGPT/Codex OAuth in `~/.codex/auth.json`. Factory Droid only sees
//! the loopback proxy and a dummy BYOK token.

use anyhow::{anyhow, Context};
use serde_json::Value;
use std::fmt::Write;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

const DEFAULT_PROXY_MODEL: &str = "codex-gpt-5.5";
const DEFAULT_CODEX_MODEL: &str = "gpt-5.5";

pub fn enabled() -> bool {
    env_flag("CODEX_PROXY_ENABLE", false) || std::env::var("CODEX_PROXY_MODEL").is_ok()
}

pub fn advertised_model() -> String {
    std::env::var("CODEX_PROXY_ADVERTISED_MODEL")
        .or_else(|_| std::env::var("CODEX_PROXY_MODEL"))
        .unwrap_or_else(|_| DEFAULT_PROXY_MODEL.to_string())
}

pub fn is_codex_model(model: &str) -> bool {
    let norm = model.trim().to_ascii_lowercase();
    enabled()
        && (norm == advertised_model().to_ascii_lowercase()
            || norm.starts_with("codex:")
            || norm.starts_with("codex-"))
}

pub async fn generate(model: &str, messages: &[Value]) -> anyhow::Result<String> {
    let prompt = render_messages(messages);
    let codex_model = resolve_codex_model(model);
    let codex_bin = std::env::var("CODEX_PROXY_BIN").unwrap_or_else(|_| "codex".to_string());
    let cwd = std::env::var("CODEX_PROXY_CWD").unwrap_or_else(|_| "/tmp".to_string());
    let timeout_secs = std::env::var("CODEX_PROXY_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v >= 5)
        .unwrap_or(180);

    let mut child = Command::new(codex_bin)
        .arg("exec")
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("--ignore-rules")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--cd")
        .arg(cwd)
        .arg("-m")
        .arg(codex_model)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("failed to run codex exec")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .context("failed to write prompt to codex stdin")?;
    } else {
        return Err(anyhow!("failed to open codex stdin"));
    }

    let output = match timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await {
        Ok(result) => result.context("failed to wait for codex exec")?,
        Err(_) => return Err(anyhow!("codex exec timed out after {timeout_secs}s")),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "codex exec failed with status {}: {}{}{}",
            output.status,
            stderr.trim(),
            if stderr.trim().is_empty() || stdout.trim().is_empty() {
                ""
            } else {
                " | "
            },
            stdout.trim()
        ));
    }

    let text = String::from_utf8(output.stdout)
        .context("codex exec returned non-utf8 stdout")?
        .trim()
        .to_string();

    if text.is_empty() {
        Err(anyhow!("codex exec returned an empty response"))
    } else {
        Ok(text)
    }
}

fn resolve_codex_model(model: &str) -> String {
    if let Ok(explicit) = std::env::var("CODEX_PROXY_CODEX_MODEL") {
        return explicit;
    }

    let norm = model.trim();
    if let Some(rest) = norm.strip_prefix("codex:") {
        return rest.to_string();
    }

    if norm == DEFAULT_PROXY_MODEL {
        return DEFAULT_CODEX_MODEL.to_string();
    }

    norm.strip_prefix("codex-")
        .map(|rest| rest.to_string())
        .unwrap_or_else(|| DEFAULT_CODEX_MODEL.to_string())
}

fn render_messages(messages: &[Value]) -> String {
    let turns = messages
        .iter()
        .rev()
        .filter_map(|m| {
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            if role != "user" && role != "assistant" {
                return None;
            }
            let content = render_content(m.get("content")?);
            if content.trim().is_empty() {
                None
            } else {
                Some((role, content))
            }
        })
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut prompt = String::from("Reply directly and concisely.\n\n");

    for (role, content) in turns {
        let trimmed = truncate_chars(content.trim(), 12_000);
        let _ = writeln!(prompt, "{role}: {trimmed}\n");
    }

    prompt
}

fn render_content(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return user_visible_text(s).unwrap_or_default();
    }

    if let Some(items) = content.as_array() {
        return items
            .iter()
            .filter_map(|item| {
                let text = item
                    .get("text")
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("content").and_then(|v| v.as_str()))?;
                user_visible_text(text)
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    let text = content.to_string();
    user_visible_text(&text).unwrap_or_default()
}

fn user_visible_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.starts_with("<system-reminder>") {
        return None;
    }

    Some(trimmed.to_string())
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }

    let mut out = s.chars().take(max_chars).collect::<String>();
    out.push_str("\n[truncated by local proxy]");
    out
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}
