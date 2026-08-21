//! ⚖️ Tool Profiles — R14
//!
//! Controls which tools are exposed to agents to reduce token cost.
//!
//! The profile names are policy, not a claim that every named upstream tool is
//! available. [`resolve_live_profile`] intersects that policy with the running
//! MCP registry before a caller receives an executable tool list.

use op_mcp::tool_registry::{ToolReadiness, ToolRegistry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolProfile {
    Minimal,
    #[default]
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

/// A profile after its policy list has been reconciled with the live registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedToolProfile {
    pub profile: ToolProfile,
    pub tools: Vec<String>,
    pub schema_tokens: u32,
    pub savings_percent: u32,
}

impl ResolvedToolProfile {
    pub fn tool_count(&self) -> u32 {
        self.tools.len() as u32
    }
}

/// Return only tools that are both selected by `profile` and executable now.
///
/// Disabled and mock entries remain visible in the operator catalog with their
/// reasons, but they must never be offered to a model as callable tools.
pub async fn resolve_live_profile(
    registry: &ToolRegistry,
    profile: ToolProfile,
) -> ResolvedToolProfile {
    let catalog = registry.catalog(0, usize::MAX, None).await;
    let selected = live_profile_entries(&catalog, profile);
    let full = live_profile_entries(&catalog, ToolProfile::Full);

    let schema_tokens = estimated_schema_tokens(&selected);
    let full_schema_tokens = estimated_schema_tokens(&full);
    let savings_percent = if full_schema_tokens == 0 {
        0
    } else {
        ((full_schema_tokens.saturating_sub(schema_tokens) * 100) / full_schema_tokens).min(100)
    };

    ResolvedToolProfile {
        profile,
        tools: selected
            .into_iter()
            .map(|entry| entry.definition.name.clone())
            .collect(),
        schema_tokens,
        savings_percent,
    }
}

fn live_profile_entries<'a>(
    catalog: &'a [op_mcp::tool_registry::ToolCatalogEntry],
    profile: ToolProfile,
) -> Vec<&'a op_mcp::tool_registry::ToolCatalogEntry> {
    let allowed = tools_for_profile(profile);
    catalog
        .iter()
        .filter(|entry| {
            matches!(&entry.readiness, ToolReadiness::Live)
                && allowed.contains(&entry.definition.name.as_str())
        })
        .collect()
}

fn estimated_schema_tokens(entries: &[&op_mcp::tool_registry::ToolCatalogEntry]) -> u32 {
    entries
        .iter()
        .map(|entry| {
            let definition = &entry.definition;
            let characters = definition.name.len()
                + definition.description.len()
                + definition.category.len()
                + definition.namespace.len()
                + definition.tags.iter().map(String::len).sum::<usize>()
                + definition.input_schema.to_string().len();
            characters.div_ceil(4) as u32
        })
        .sum()
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
    use anyhow::Result;
    use async_trait::async_trait;
    use op_mcp::tool_registry::{Tool, ToolReadiness};
    use simd_json::{json, OwnedValue as Value};
    use std::sync::Arc;

    struct ProfileTool {
        name: &'static str,
        readiness: ToolReadiness,
    }

    #[async_trait]
    impl Tool for ProfileTool {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "profile test tool"
        }

        fn input_schema(&self) -> Value {
            json!({"type": "object", "properties": {"query": {"type": "string"}}})
        }

        fn readiness(&self) -> ToolReadiness {
            self.readiness.clone()
        }

        async fn execute(&self, input: Value) -> Result<Value> {
            Ok(input)
        }
    }

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

    #[tokio::test]
    async fn live_profile_excludes_unavailable_and_unselected_tools() {
        let registry = ToolRegistry::new();
        for tool in [
            ProfileTool {
                name: "ask_question",
                readiness: ToolReadiness::Live,
            },
            ProfileTool {
                name: "list_notebooks",
                readiness: ToolReadiness::Disabled {
                    reason: "NotebookLM sidecar is offline".to_string(),
                },
            },
            ProfileTool {
                name: "doctor",
                readiness: ToolReadiness::Live,
            },
            ProfileTool {
                name: "unrelated_live_tool",
                readiness: ToolReadiness::Live,
            },
        ] {
            registry
                .register(Arc::new(tool))
                .await
                .expect("register profile test tool");
        }

        let minimal = resolve_live_profile(&registry, ToolProfile::Minimal).await;
        assert_eq!(minimal.tools, ["ask_question"]);
        assert_eq!(minimal.tool_count(), 1);

        let full = resolve_live_profile(&registry, ToolProfile::Full).await;
        assert_eq!(full.tools, ["ask_question", "doctor"]);
        assert_eq!(full.tool_count(), 2);
        assert!(full.schema_tokens > 0);
        assert!(minimal.savings_percent > 0);
    }
}
