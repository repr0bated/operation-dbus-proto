//! Deterministic audience and tool-set policy for the single MCP ingress.
//!
//! These documents contain projection policy only. They never contain grants,
//! identity assertions, session anchors, or footprint/hash material.

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_AUDIENCE_POLICY_PATH: &str = "/etc/opdbus/mcp-audience-policy.json";
pub const DEFAULT_TOOLSETS_PATH: &str = "/etc/opdbus/mcp-toolsets.json";

pub const COMPACT_TOOL_NAMES: [&str; 4] = [
    "list_tools",
    "search_tools",
    "get_tool_schema",
    "execute_tool",
];

pub const HOT_TOOL_NAMES: [&str; 5] = [
    "memory_recall",
    "memory_store",
    "workflow_query",
    "workflow_run",
    "toolsets",
];

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudiencePolicy {
    pub version: u64,
    pub rotation_epoch: u64,
    pub singleton_chatbot_principal_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolTemperature {
    Warm,
    Cold,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolsetDefinition {
    pub id: String,
    pub temperature: ToolTemperature,
    pub provider: String,
    pub requires_provider_health: bool,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolsetManifest {
    pub generation: u64,
    pub hot: Vec<String>,
    pub sets: Vec<ToolsetDefinition>,
}

#[derive(Debug, Clone)]
pub struct McpProjectionPolicy {
    pub audience: AudiencePolicy,
    pub toolsets: ToolsetManifest,
}

impl McpProjectionPolicy {
    pub fn load_from_env() -> Result<Self> {
        let audience_path = std::env::var_os("OP_MCP_AUDIENCE_POLICY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_AUDIENCE_POLICY_PATH));
        let toolsets_path = std::env::var_os("OP_MCP_TOOLSETS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_TOOLSETS_PATH));
        Self::load_protected(&audience_path, &toolsets_path)
    }

    pub fn load_protected(audience_path: &Path, toolsets_path: &Path) -> Result<Self> {
        let audience_bytes = read_protected(audience_path)?;
        let toolsets_bytes = read_protected(toolsets_path)?;
        let audience = parse_audience_policy(&audience_bytes)?;
        let toolsets = parse_toolset_manifest(&toolsets_bytes)?;
        Ok(Self { audience, toolsets })
    }

    pub fn is_singleton_chatbot(&self, principal_id: &str) -> bool {
        principal_id == self.audience.singleton_chatbot_principal_id
    }

    pub fn toolset(&self, id: &str) -> Option<&ToolsetDefinition> {
        self.toolsets.sets.iter().find(|set| set.id == id)
    }
}

pub fn parse_audience_policy(bytes: &[u8]) -> Result<AudiencePolicy> {
    let policy: AudiencePolicy =
        serde_json::from_slice(bytes).context("MCP audience policy is not valid JSON")?;
    if policy.version == 0 {
        bail!("MCP audience policy version must be positive");
    }
    if policy.rotation_epoch == 0 {
        bail!("MCP audience rotation_epoch must be positive");
    }
    let parsed = uuid::Uuid::parse_str(&policy.singleton_chatbot_principal_id)
        .context("singleton_chatbot_principal_id must be a UUID")?;
    if parsed.to_string() != policy.singleton_chatbot_principal_id {
        bail!("singleton_chatbot_principal_id must use canonical lowercase UUID form");
    }
    Ok(policy)
}

pub fn parse_toolset_manifest(bytes: &[u8]) -> Result<ToolsetManifest> {
    let manifest: ToolsetManifest =
        serde_json::from_slice(bytes).context("MCP tool-set manifest is not valid JSON")?;
    if manifest.generation == 0 {
        bail!("MCP tool-set generation must be positive");
    }
    if manifest.hot.as_slice() != HOT_TOOL_NAMES {
        bail!(
            "MCP HOT surface must be exactly: {}",
            HOT_TOOL_NAMES.join(", ")
        );
    }

    let mut set_ids = HashSet::new();
    for set in &manifest.sets {
        if !valid_identifier(&set.id) {
            bail!("invalid MCP tool-set id '{}'", set.id);
        }
        if !set_ids.insert(set.id.as_str()) {
            bail!("duplicate MCP tool-set id '{}'", set.id);
        }
        if !valid_provider_id(&set.provider) {
            bail!("invalid MCP tool-set provider '{}'", set.provider);
        }
        if set.tools.is_empty() {
            bail!("MCP tool-set '{}' contains no typed tools", set.id);
        }
        let mut tools = HashSet::new();
        for tool in &set.tools {
            if !valid_typed_tool_name(tool) {
                bail!(
                    "MCP tool-set '{}' has non-canonical tool '{}'",
                    set.id,
                    tool
                );
            }
            if prohibited_external_tool(tool) {
                bail!(
                    "MCP tool-set '{}' contains generic/compact tool '{}'",
                    set.id,
                    tool
                );
            }
            if !tools.insert(tool.as_str()) {
                bail!("MCP tool-set '{}' repeats tool '{}'", set.id, tool);
            }
        }
    }
    Ok(manifest)
}

fn read_protected(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("MCP policy file is unreadable: {}", path.display()))?;
    if !metadata.is_file() {
        bail!("MCP policy path is not a regular file: {}", path.display());
    }
    if metadata.uid() != 0 {
        bail!("MCP policy file must be root-owned: {}", path.display());
    }
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        bail!(
            "MCP policy file must not be group/world accessible (mode {:o}): {}",
            mode,
            path.display()
        );
    }
    fs::read(path).with_context(|| format!("read MCP policy file {}", path.display()))
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_provider_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

fn valid_typed_tool_name(value: &str) -> bool {
    let mut parts = value.split('.');
    parts.next() == Some("plugin") && parts.clone().count() == 2 && parts.all(valid_identifier)
}

fn prohibited_external_tool(value: &str) -> bool {
    let leaf = value.rsplit('.').next().unwrap_or(value);
    COMPACT_TOOL_NAMES.contains(&leaf)
        || matches!(leaf, "invoke_tool" | "respond" | "register_tool")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_hot_manifest_is_accepted() {
        let manifest = parse_toolset_manifest(
            br#"{
              "generation": 7,
              "hot": ["memory_recall", "memory_store", "workflow_query", "workflow_run", "toolsets"],
              "sets": [{
                "id": "emqx_ops",
                "temperature": "warm",
                "provider": "emqx",
                "requires_provider_health": true,
                "tools": ["plugin.emqx.get_status"]
              }]
            }"#,
        )
        .unwrap();
        assert_eq!(manifest.generation, 7);
    }

    #[test]
    fn frequency_or_generic_tools_cannot_change_the_surface() {
        let dynamic_hot = br#"{
          "generation": 1,
          "hot": ["memory_recall", "memory_store", "workflow_query", "invoke_tool", "toolsets"],
          "sets": []
        }"#;
        assert!(parse_toolset_manifest(dynamic_hot).is_err());

        let generic_set = br#"{
          "generation": 1,
          "hot": ["memory_recall", "memory_store", "workflow_query", "workflow_run", "toolsets"],
          "sets": [{
            "id": "escape",
            "temperature": "cold",
            "provider": "emqx",
            "requires_provider_health": false,
            "tools": ["plugin.cognitive_mcp.invoke_tool"]
          }]
        }"#;
        assert!(parse_toolset_manifest(generic_set).is_err());
    }

    #[test]
    fn audience_is_one_exact_principal() {
        let policy = parse_audience_policy(
            br#"{
              "version": 1,
              "rotation_epoch": 2,
              "singleton_chatbot_principal_id": "87b0decc-8464-5abf-05d8-b52ec88ff9f1"
            }"#,
        )
        .unwrap();
        assert_eq!(policy.rotation_epoch, 2);

        assert!(parse_audience_policy(
            br#"{
              "version": 1,
              "rotation_epoch": 2,
              "singleton_chatbot_principal_id": "chatbot-*"
            }"#
        )
        .is_err());
    }

    #[test]
    fn deployed_manifest_is_valid_and_keeps_provider_tools_warm() {
        let manifest = parse_toolset_manifest(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../deploy/config/mcp-toolsets.json"
        )))
        .unwrap();
        assert_eq!(manifest.hot.as_slice(), HOT_TOOL_NAMES);
        assert_eq!(manifest.generation, 3);
        assert!(manifest
            .sets
            .iter()
            .any(|set| set.id == "notebooklm_research"
                && set.provider == "notebooklm-mcp-authenticated"));
        assert!(manifest
            .sets
            .iter()
            .any(|set| set.id == "mongodb_data"
                && set.provider == "mongodb-mcp-server-authenticated"));
        assert!(manifest.sets.iter().all(|set| {
            set.tools
                .iter()
                .all(|tool| !manifest.hot.iter().any(|hot| hot == tool))
        }));
    }
}
