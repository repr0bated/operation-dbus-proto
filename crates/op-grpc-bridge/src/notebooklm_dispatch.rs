//! NotebookLM method dispatch.
//!
//! Bridges the `notebooklm` D-Bus/gRPC plugin surface (what `zcall notebooklm
//! <method>` hits) to the live NotebookLM MCP sidecar managed by
//! `op-cognitive-mcp`. Without this the method call falls through the generic
//! path in [`crate::mutation_engine`] and merely echoes its arguments.
//!
//! The plugin method names are modeled on the sidecar's tool names, so
//! resolution is a normalized match (underscores/hyphens/prefix insensitive)
//! against the sidecar's published tools, with a small alias table for the few
//! cases where the plugin verb differs from the tool verb.

use anyhow::{anyhow, Result};
use serde_json::Value;

/// High-confidence aliases where the plugin method verb differs from the
/// NotebookLM sidecar tool verb.
fn alias_for(method: &str) -> Option<&'static str> {
    match method {
        "query_notebook" => Some("ask_question"),
        "cross_notebook_query" => Some("ask_question"),
        "search_notebooks" => Some("search"),
        "get_library_stats" => Some("get_stats"),
        _ => None,
    }
}

fn normalize(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn resolve_tool(method: &str, available: &[String]) -> Option<String> {
    let target = normalize(method);
    if let Some(found) = available.iter().find(|t| normalize(t) == target) {
        return Some(found.clone());
    }
    if let Some(alias) = alias_for(method) {
        let alias_norm = normalize(alias);
        if let Some(found) = available.iter().find(|t| normalize(t) == alias_norm) {
            return Some(found.clone());
        }
    }
    None
}

/// Dispatch a `notebooklm` method to the NotebookLM sidecar.
///
/// `args` is the caller's method arguments (already unwrapped from the zcall
/// one-element array by the mutation engine). Returns a JSON envelope carrying
/// the resolved tool name and the sidecar's raw result.
pub async fn dispatch_notebooklm_method(method: &str, args: &Value) -> Result<Value> {
    let available = op_cognitive_mcp::notebooklm::list_notebooklm_tools()
        .await
        .map_err(|e| anyhow!("NotebookLM sidecar unavailable: {e}"))?;

    let tool = resolve_tool(method, &available).ok_or_else(|| {
        anyhow!(
            "no NotebookLM tool matches method '{method}'; available tools: [{}]",
            available.join(", ")
        )
    })?;

    let result = op_cognitive_mcp::notebooklm::call_notebooklm_tool(&tool, args.clone()).await?;

    Ok(serde_json::json!({
        "success": true,
        "method": method,
        "tool": tool,
        "result": result,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_exact_and_normalized_names() {
        let available = vec![
            "list_notebooks".to_string(),
            "add-source-url".to_string(),
            "ask_question".to_string(),
        ];
        assert_eq!(
            resolve_tool("list_notebooks", &available).as_deref(),
            Some("list_notebooks")
        );
        assert_eq!(
            resolve_tool("add_source_url", &available).as_deref(),
            Some("add-source-url")
        );
    }

    #[test]
    fn resolves_via_alias() {
        let available = vec!["ask_question".to_string()];
        assert_eq!(
            resolve_tool("query_notebook", &available).as_deref(),
            Some("ask_question")
        );
    }

    #[test]
    fn unknown_method_returns_none() {
        let available = vec!["list_notebooks".to_string()];
        assert!(resolve_tool("nonexistent_method", &available).is_none());
    }
}
