//! Read zeroclaw `configurable_options` from the sealed blob catalog (Absolute Base)
//! and execute provisioning steps declared in that contract.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::{ValueAsScalar, ValueObjectAccess};
use simd_json::OwnedValue as SimdValue;
use tracing::warn;

/// Subset of zeroclaw `ConfigurableOptions` needed at provision time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroclawConfigurableOptions {
    #[serde(default)]
    pub user_container: UserContainerOptions,
    #[serde(default)]
    pub identity_chain: IdentityOptions,
    #[serde(default)]
    pub memory_namespaces: Vec<MemoryNamespaceOption>,
    #[serde(default)]
    pub privacy_policy: PrivacyOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserContainerOptions {
    #[serde(default)]
    pub source_script: String,
    #[serde(default)]
    pub container_id_template: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub cognitive_mcp_endpoint: String,
    #[serde(default)]
    pub feature_flags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityOptions {
    #[serde(default)]
    pub mac_address_key: String,
    #[serde(default)]
    pub pre_shared_key: String,
    #[serde(default)]
    pub wireguard_pubkey: String,
    #[serde(default)]
    pub mcp_token_derivation: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryNamespaceOption {
    #[serde(default)]
    pub namespace_template: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub identity_link_key: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrivacyOptions {
    #[serde(default)]
    pub email_storage_rule: String,
    #[serde(default)]
    pub sensitive_fields: Vec<String>,
}

/// Feature toggles materialized from schema + env for a single provision run.
#[derive(Debug, Clone)]
pub struct ProvisionFeatures {
    pub ghostbridge: bool,
    pub semantic_search: bool,
}

impl Default for ZeroclawConfigurableOptions {
    fn default() -> Self {
        Self {
            user_container: UserContainerOptions {
                source_script: "deploy/scripts/provision-workspace-subscriber.sh".to_string(),
                container_id_template: "{container_id}".to_string(),
                image: "images:debian/12".to_string(),
                cognitive_mcp_endpoint: "http://127.0.0.1:3003/mcp".to_string(),
                feature_flags: vec!["ghostbridge".to_string(), "semantic_search".to_string()],
            },
            identity_chain: IdentityOptions {
                mac_address_key: "--mac".to_string(),
                pre_shared_key: "--psk".to_string(),
                wireguard_pubkey: "container:{container_id}:identity/wireguard_pubkey".to_string(),
                mcp_token_derivation: "uuid_v5(wireguard_pubkey)".to_string(),
            },
            memory_namespaces: default_memory_namespaces(),
            privacy_policy: PrivacyOptions {
                email_storage_rule: "Store email only when GhostBridge is disabled; GhostBridge users withhold email from CozoDB.".to_string(),
                sensitive_fields: vec![
                    "token".to_string(),
                    "wireguard_public_key".to_string(),
                    "wireguard_config".to_string(),
                    "public_key".to_string(),
                    "mcp_token".to_string(),
                    "mac_address".to_string(),
                    "psk".to_string(),
                ],
            },
        }
    }
}

fn default_memory_namespaces() -> Vec<MemoryNamespaceOption> {
    vec![
        MemoryNamespaceOption {
            namespace_template: "container:{container_id}:identity".to_string(),
            keys: vec![
                "wireguard_pubkey".to_string(),
                "mcp_token".to_string(),
                "mac_address".to_string(),
                "psk".to_string(),
                "email".to_string(),
            ],
            purpose: "Container identity namespace. Email is present only when GhostBridge is disabled.".to_string(),
            identity_link_key: "container:{container_id}:identity.wireguard_pubkey".to_string(),
        },
        MemoryNamespaceOption {
            namespace_template: "container:{container_id}:soul".to_string(),
            keys: vec!["profile".to_string()],
            purpose: "Container soul profile bound to the same container identity.".to_string(),
            identity_link_key: String::new(),
        },
        MemoryNamespaceOption {
            namespace_template: "container:{container_id}:domain:work".to_string(),
            keys: vec!["MEMORY_INDEX".to_string()],
            purpose: "Work-domain namespace bound to the container identity.".to_string(),
            identity_link_key: String::new(),
        },
        MemoryNamespaceOption {
            namespace_template: "container:{container_id}:domain:personal".to_string(),
            keys: vec!["MEMORY_INDEX".to_string()],
            purpose: "Personal-domain namespace bound to the container identity.".to_string(),
            identity_link_key: String::new(),
        },
        MemoryNamespaceOption {
            namespace_template: "container:{container_id}:domain:home".to_string(),
            keys: vec!["MEMORY_INDEX".to_string()],
            purpose: "Home-domain namespace bound to the container identity.".to_string(),
            identity_link_key: String::new(),
        },
        MemoryNamespaceOption {
            namespace_template: "container:{container_id}:index".to_string(),
            keys: vec!["MEMORY_INDEX".to_string()],
            purpose: "Namespace index for identity, soul, and domain namespaces.".to_string(),
            identity_link_key: String::new(),
        },
        MemoryNamespaceOption {
            namespace_template: "container:{container_id}:features".to_string(),
            keys: vec!["ghostbridge".to_string(), "semantic_search".to_string()],
            purpose: "Feature flags projected for the container identity.".to_string(),
            identity_link_key: String::new(),
        },
    ]
}

/// Load configurable options from the zeroclaw sealed blob; fall back to typed defaults.
pub fn load_configurable_options() -> ZeroclawConfigurableOptions {
    match load_configurable_options_from_shm() {
        Some(options) => options,
        None => {
            warn!("zeroclaw configurable_options not found in SHM; using built-in defaults");
            ZeroclawConfigurableOptions::default()
        }
    }
}

fn load_configurable_options_from_shm() -> Option<ZeroclawConfigurableOptions> {
    let schema = op_blob::catalog::read_plugin_schema_shm("zeroclaw")?;
    if let Some(example) = schema.example {
        if let Ok(options) = parse_configurable_options_value(&example) {
            return Some(options);
        }
    }
    schema
        .fields
        .get("configurable_options")
        .and_then(|field| field.default.as_ref())
        .and_then(|value| parse_configurable_options_value(value).ok())
}

fn parse_configurable_options_value(root: &SimdValue) -> Result<ZeroclawConfigurableOptions> {
    let node = root
        .get("configurable_options")
        .unwrap_or(root);
    let json = simd_json::serde::to_string(node).context("serialize configurable_options")?;
    serde_json::from_str(&json).context("parse zeroclaw configurable_options")
}

/// Resolve feature flags for a provision run (schema declares options; env selects defaults).
pub fn resolve_provision_features(options: &ZeroclawConfigurableOptions) -> ProvisionFeatures {
    let declared: std::collections::HashSet<_> =
        options.user_container.feature_flags.iter().map(String::as_str).collect();

    let ghostbridge = env_flag("PROVISION_GHOSTBRIDGE", true)
        && declared.contains("ghostbridge");
    let semantic_search = env_flag("PROVISION_SEMANTIC_SEARCH", false)
        && declared.contains("semantic_search");

    ProvisionFeatures {
        ghostbridge,
        semantic_search,
    }
}

fn env_flag(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes"
        })
        .unwrap_or(default)
}

/// Whether email must be withheld from durable stores for this provision run.
pub fn withhold_email(options: &ZeroclawConfigurableOptions, features: &ProvisionFeatures) -> bool {
    features.ghostbridge
        || options
            .privacy_policy
            .email_storage_rule
            .to_ascii_lowercase()
            .contains("ghostbridge")
}

/// Substitute `{container_id}` and `{username}` in schema templates.
pub fn substitute_template(template: &str, container_id: &str) -> String {
    template
        .replace("{container_id}", container_id)
        .replace("{username}", container_id)
}

/// Incus container name from `user_container.container_id_template`.
pub fn resolve_container_name(template: &str, container_id: &str) -> String {
    let resolved = substitute_template(template, container_id);
    if resolved.is_empty() || resolved == template && !template.contains('{') {
        return sanitize_container_name(container_id);
    }
    sanitize_container_name(&resolved)
}

fn sanitize_container_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if sanitized.is_empty() {
        return "workspace".to_string();
    }
    if sanitized.len() > 63 {
        sanitized[..63].trim_end_matches('-').to_string()
    } else {
        sanitized
    }
}

/// Default agent model routing from zeroclaw live state in the blob (independent of chatbot).
pub fn default_agent_model_from_shm() -> Option<(String, String)> {
    let schema = op_blob::catalog::read_plugin_schema_shm("zeroclaw")?;
    let state = schema.example?;
    let provider = state
        .get("selected_provider")
        .and_then(|v| v.as_str())
        .map(str::to_string)?;
    let model = state
        .get("selected_model")
        .and_then(|v| v.as_str())
        .map(str::to_string)?;
    Some((provider, model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_container_id_in_namespace_template() {
        let ns = substitute_template("container:{container_id}:soul", "abc-123");
        assert_eq!(ns, "container:abc-123:soul");
    }

    #[test]
    fn resolves_container_name_from_template() {
        let name = resolve_container_name("ws-{username}", "sess-1");
        assert_eq!(name, "ws-sess-1");
    }

    #[test]
    fn ghostbridge_withholds_email() {
        let options = ZeroclawConfigurableOptions::default();
        let features = ProvisionFeatures {
            ghostbridge: true,
            semantic_search: false,
        };
        assert!(withhold_email(&options, &features));
    }
}
