//! Plugin loader - discovers state plugins and instantiates them
//!
//! This module deliberately does not maintain a second plugin list. If a state
//! plugin exists in `state_plugins`, the registry discovers it and registers it.
//!
//! ## Schema Validation
//!
//! Plugins are validated against JSON schema files at runtime.
//! If a schema file is missing in strict mode, the plugin will be rejected.
//! Schema files are loaded from `schemas/plugin/{name}.json`.
//!
//! ## Canonical Path Enforcement
//!
//! All plugins must use canonical D-Bus paths:
//! - Path: `/org/opdbus/v1/plugins/{name}`
//! - Interface: `org.opdbus.v1.Plugin.Plugins.{Name}`
//!
//! Legacy paths are deprecated and will be rejected.

use anyhow::{anyhow, Result};
use op_state_store::StateStore;
use simd_json::prelude::*;
use std::sync::Arc;

use crate::schema_loader::SchemaLoader;

use crate::state_plugins::{
    AdcPlugin, AgentConfigPlugin, AntigravityChatPlugin, AntigravityPlugin, BtrfsPlugin,
    CognitiveMcpPlugin, CompactMcpPlugin, ConfigPlugin, CronPlugin, CtlPlaneChatbotPlugin,
    DnsResolverPlugin, EndpointPlugin, FactoryPlugin, Fail2banPlugin, FreeDesktopPlugin,
    FullSystemPlugin, GcloudAdcPlugin, HardwarePlugin, IncusPlugin, KeypairPlugin, KeyringPlugin,
    KnowledgePlugin, Login1Plugin, LxcPlugin, MailServerPlugin, McpStatePlugin, MemoryPlugin,
    NetStatePlugin, NetmakerConfig, NetmakerPlugin, NotebookLmPlugin, OciPlugin,
    OpenFlowObfuscationPlugin, OpenFlowPlugin, OscalSubidRegistryPlugin, OvsBridgePlugin,
    OvsdbDaemonPlugin, PackageKitPlugin, PciDeclPlugin, PrivacyRouterPlugin, PrivacyRoutesPlugin,
    ProcfsPlugin, ProxmoxPlugin, ProxyServerPlugin, RovsCommandsPlugin, RtnetlinkPlugin,
    S6StatePlugin, S6SystemctlPlugin, SchemaRendererPlugin, ServicePlugin, SessDeclPlugin,
    SoftwarePlugin, UnixSocketPlugin, UsersPlugin, WebUiPlugin, WgcfPlugin, WireGuardPlugin,
    WorkflowsPlugin, XrayPlugin, ZeroclawPlugin,
};
use crate::AutoPlugin;

/// Default plugin loader configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginRegistryConfig {
    /// Deprecated runtime policy field. Registration is discovery-based.
    #[serde(default = "default_auto_load")]
    pub auto_load: Vec<String>,

    /// Plugin-specific configurations
    #[serde(default)]
    pub plugin_configs: std::collections::HashMap<String, simd_json::OwnedValue>,
}

fn default_auto_load() -> Vec<String> {
    Vec::new()
}

fn discover_pub_mod_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("pub mod ")?;
    let name = rest.strip_suffix(';')?.trim();
    if name.is_empty() {
        return None;
    }
    Some(module_name_to_plugin_name(name))
}

fn is_state_plugin_helper_module(name: &str) -> bool {
    matches!(name, "plugin_schema_defs" | "schema_contract")
}

fn module_name_to_plugin_name(name: &str) -> String {
    name.strip_suffix("_plugin").unwrap_or(name).to_string()
}

impl Default for PluginRegistryConfig {
    fn default() -> Self {
        Self {
            auto_load: default_auto_load(),
            plugin_configs: std::collections::HashMap::new(),
        }
    }
}

/// Default plugin loader.
///
/// This is intentionally not the authoritative plugin catalog. Its job is to
/// instantiate the built-in plugin implementations so the real catalog path can
/// persist their canonical plugin documents during registration.
pub struct DefaultPluginRegistry {
    config: PluginRegistryConfig,
    state_store: Arc<dyn StateStore>,
    schema_loader: SchemaLoader,
}

impl DefaultPluginRegistry {
    /// Create a new plugin loader
    pub fn new(state_store: Arc<dyn StateStore>) -> Self {
        Self {
            config: PluginRegistryConfig::default(),
            state_store,
            schema_loader: SchemaLoader::new("schemas/plugin"),
        }
    }

    /// Create with custom schema directory
    pub fn with_schema_dir(
        state_store: Arc<dyn StateStore>,
        schema_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            config: PluginRegistryConfig::default(),
            state_store,
            schema_loader: SchemaLoader::new(schema_dir),
        }
    }

    /// Validate a plugin has a valid schema file
    ///
    /// Returns Ok(()) if schema exists and is valid
    /// Returns Err if schema is missing or invalid (in strict mode)
    pub async fn validate_plugin_schema(&self, plugin_name: &str) -> Result<()> {
        match self.schema_loader.load_schema(plugin_name).await? {
            Some(_) => Ok(()),
            None => Err(anyhow!(
                "Plugin '{}' has no valid schema file. Expected: schemas/plugin/{}.json",
                plugin_name,
                plugin_name
            )),
        }
    }

    /// Check if a schema file exists for a plugin
    pub async fn schema_exists(&self, plugin_name: &str) -> bool {
        self.schema_loader.schema_exists(plugin_name).await
    }

    /// Create with custom configuration
    pub fn with_config(state_store: Arc<dyn StateStore>, config: PluginRegistryConfig) -> Self {
        Self {
            config,
            state_store,
            schema_loader: SchemaLoader::new("schemas/plugin"),
        }
    }

    /// Create with custom configuration and schema directory
    pub fn with_config_and_schema_dir(
        state_store: Arc<dyn StateStore>,
        config: PluginRegistryConfig,
        schema_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            config,
            state_store,
            schema_loader: SchemaLoader::new(schema_dir),
        }
    }

    /// Resolve user/request-facing plugin references into canonical loader names.
    ///
    /// Supports direct names, aliases, and projection paths like
    /// `/org/opdbus/v1/plugins/<plugin>/...`.
    ///
    /// The old `/org/opdbus/v1/plugin/plugins/...` spelling is accepted as a
    /// compatibility alias and normalized to the canonical plural base path.
    pub fn resolve_requested_plugin_name(requested: &str) -> Result<String> {
        let trimmed = requested.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("Plugin identifier cannot be empty"));
        }

        let extracted = Self::extract_plugin_name_from_projection_path(trimmed);
        let from_path = extracted.as_deref().unwrap_or(trimmed);
        let normalized = from_path
            .trim()
            .trim_matches('/')
            .replace('-', "_")
            .to_lowercase();

        let canonical = match normalized.as_str() {
            "network" => "net",
            "systemd" | "dinit" | "service_s6" => "s6",
            "web_ui" | "webui" => "web_ui",
            "mail" | "mailserver" => "mail_server",
            "privacyroutes" => "privacy_routes",
            "privacyrouter" => "privacy_router",
            "ovsbridge" => "ovsdb_bridge",
            "rtnet" => "rtnetlink",
            "sessdecl" => "sess_decl",
            other => other,
        };

        Ok(canonical.to_string())
    }

    fn extract_plugin_name_from_projection_path(requested: &str) -> Option<String> {
        // Normalize aliases first, then extract the top-level plugin name.
        let normalized = crate::canonical::normalize_plugin_path(requested)?;
        normalized
            .strip_prefix(crate::canonical::PLUGIN_BASE_PATH)?
            .trim_start_matches('/')
            .split('/')
            .find(|segment| !segment.is_empty())
            .map(|segment| segment.to_string())
    }

    /// Compatibility shim for older callers. Registration is discovery-based:
    /// if a state plugin exists, it is loaded.
    pub async fn load_default_plugins(&self) -> Result<Vec<Arc<dyn op_state::StatePlugin>>> {
        self.load_all_plugins().await
    }

    /// Load every built-in plugin implementation so the schema catalog can
    /// publish the full plugin contract set, not just runtime autoload state.
    pub async fn load_all_plugins(&self) -> Result<Vec<Arc<dyn op_state::StatePlugin>>> {
        let mut plugins: Vec<Arc<dyn op_state::StatePlugin>> = Vec::new();
        let mut plugin_names = Self::available_plugins();

        plugin_names.sort();
        plugin_names.dedup();

        for plugin_name in plugin_names {
            match self.load_plugin(&plugin_name).await {
                Ok(plugin) => {
                    if !plugin.is_available() {
                        tracing::info!(
                            "Loaded unavailable plugin schema {}: {}",
                            plugin_name,
                            plugin.unavailable_reason()
                        );
                    } else {
                        tracing::info!("Loaded plugin schema: {}", plugin_name);
                    }
                    plugins.push(plugin);
                }
                Err(e) => {
                    tracing::warn!("Failed to load plugin schema {}: {}", plugin_name, e);
                }
            }
        }

        tracing::info!("Loaded {} built-in plugin schemas", plugins.len());
        Ok(plugins)
    }

    /// Load a specific plugin by name
    pub async fn load_plugin(&self, name: &str) -> Result<Arc<dyn op_state::StatePlugin>> {
        let resolved_name = Self::resolve_requested_plugin_name(name)?;
        let plugin: Arc<dyn op_state::StatePlugin> = match resolved_name.as_str() {
            "mcp" => {
                let config_path =
                    self.get_plugin_config_path("mcp", "/etc/op-dbus/mcp-config.json");
                Arc::new(McpStatePlugin::new(self.state_store.clone(), config_path))
            }
            "zeroclaw" => Arc::new(ZeroclawPlugin::new()),
            "factory" => Arc::new(FactoryPlugin::new()),
            "fail2ban" => Arc::new(Fail2banPlugin::new()),
            "cron" => Arc::new(CronPlugin::new()),
            "memory" => Arc::new(MemoryPlugin::new()),
            "workflows" => Arc::new(WorkflowsPlugin::new()),
            "btrfs" => Arc::new(BtrfsPlugin::new()),
            "knowledge" => Arc::new(KnowledgePlugin::new()),
            "notebooklm" => Arc::new(NotebookLmPlugin::new()),
            "antigravity_chat" => Arc::new(AntigravityChatPlugin::new()),
            "antigravity" => Arc::new(AntigravityPlugin::new()),
            "schema_renderer" => Arc::new(SchemaRendererPlugin::new()),
            "full_system" => Arc::new(FullSystemPlugin::new()),
            "login1" => Arc::new(Login1Plugin::new()),
            "dnsresolver" => Arc::new(DnsResolverPlugin::new()),
            "keyring" => Arc::new(KeyringPlugin::new()),
            "packagekit" => Arc::new(PackageKitPlugin::new()),
            "pcidecl" => Arc::new(PciDeclPlugin::new()),
            "config" => {
                let config_path =
                    self.get_plugin_config_path("config", "/etc/op-dbus/config-store.json");
                Arc::new(ConfigPlugin::new(config_path))
            }
            "freedesktop" => Arc::new(FreeDesktopPlugin::new()),
            "cognitive_mcp" => Arc::new(CognitiveMcpPlugin::new()),
            "compact_mcp" => Arc::new(CompactMcpPlugin::new()),
            "ctl_plane_chatbot" => Arc::new(CtlPlaneChatbotPlugin::new()),
            "s6" => Arc::new(S6StatePlugin::new()),
            "s6_systemctl" => Arc::new(S6SystemctlPlugin::new()),
            "incus" => Arc::new(IncusPlugin::new()),
            "mail_server" => Arc::new(MailServerPlugin::new()),
            "unix_socket" => Arc::new(UnixSocketPlugin::new()),
            "wgcf" => Arc::new(WgcfPlugin::new(
                crate::state_plugins::wgcf::WgcfConfig::default(),
            )),
            "xray" => Arc::new(XrayPlugin::new(
                crate::state_plugins::xray::XrayConfig::default(),
            )),
            "net" => Arc::new(NetStatePlugin::new()),
            "openflow" => Arc::new(OpenFlowPlugin::new()),
            "openflow_obfuscation" => Arc::new(OpenFlowObfuscationPlugin::new(Default::default())),
            "privacy_router" => {
                let _config_path = self
                    .get_plugin_config_path("privacy_router", "/etc/op-dbus/privacy-config.json");
                use crate::state_plugins::privacy_router::PrivacyRouterConfig;
                Arc::new(PrivacyRouterPlugin::new(PrivacyRouterConfig::default()))
            }
            "proxmox" => Arc::new(ProxmoxPlugin::new()),
            "hardware" => Arc::new(HardwarePlugin::new()),
            "software" => Arc::new(SoftwarePlugin::new()),
            "users" => Arc::new(UsersPlugin::new()),
            "gcloud_adc" => Arc::new(GcloudAdcPlugin::new()),
            "keypair" => Arc::new(KeypairPlugin::new()),
            "service" => Arc::new(ServicePlugin::new()),
            "wireguard" => Arc::new(WireGuardPlugin::new()),
            "agent_config" => Arc::new(AgentConfigPlugin::new()),
            "ovsdb_bridge" => Arc::new(OvsBridgePlugin::new()),
            "ovsdb_daemon" => Arc::new(OvsdbDaemonPlugin::new()),
            "privacy_routes" => Arc::new(PrivacyRoutesPlugin::default()),
            "procfs" => Arc::new(ProcfsPlugin::new()),
            "rovs_commands" => Arc::new(RovsCommandsPlugin::new()),
            "rtnetlink" => Arc::new(RtnetlinkPlugin::new()),
            "sess_decl" => Arc::new(SessDeclPlugin::new()),
            "lxc" => Arc::new(LxcPlugin::new()),
            "netmaker" => Arc::new(NetmakerPlugin::new(NetmakerConfig::default())),
            "oci" => Arc::new(OciPlugin::new()),
            "oscal_subid_registry" => Arc::new(OscalSubidRegistryPlugin::new()),
            "adc" => Arc::new(AdcPlugin::new()),
            "endpoint" => Arc::new(EndpointPlugin::new()),
            "proxy_server" => Arc::new(ProxyServerPlugin::new()),
            "web_ui" => Arc::new(WebUiPlugin::new()),
            _ => {
                let requested_info = format!("requested='{}' resolved='{}'", name, resolved_name);
                tracing::warn!(
                    "Unknown plugin '{}'; auto-creating review-required draft from requested info",
                    name
                );
                Arc::new(
                    AutoPlugin::create_from_requested_info(&resolved_name, &requested_info).await,
                )
            }
        };

        Ok(plugin)
    }

    /// Get plugin-specific config value or default
    fn get_plugin_config_path(&self, plugin_name: &str, default: &str) -> String {
        self.config
            .plugin_configs
            .get(plugin_name)
            .and_then(|v| v.get("config_path"))
            .and_then(|v| v.as_str())
            .unwrap_or(default)
            .to_string()
    }

    /// Discover plugins from the state-plugin module tree.
    ///
    /// This is intentionally not a hand-maintained plugin catalog. The module
    /// tree is the source of discoverable plugin subjects; unknown subjects
    /// still flow through `load_plugin()` and the auto-create fallback.
    pub fn available_plugins() -> Vec<String> {
        const STATE_PLUGIN_MODS: &str = include_str!("state_plugins/mod.rs");

        let mut plugins = STATE_PLUGIN_MODS
            .lines()
            .filter_map(discover_pub_mod_name)
            .filter(|name| !is_state_plugin_helper_module(name))
            .collect::<Vec<_>>();

        plugins.sort();
        plugins.dedup();
        plugins
    }

    /// Get list of plugins with valid schema files
    pub async fn available_schemas(&self) -> Result<Vec<String>> {
        self.schema_loader.list_available_schemas().await
    }

    /// Load schema for a plugin
    pub async fn load_plugin_schema(
        &self,
        plugin_name: &str,
    ) -> Result<Option<op_state::PluginSchema>> {
        self.schema_loader.load_schema(plugin_name).await
    }

    /// Check if a plugin is enabled for auto-load
    pub fn is_auto_load(&self, plugin_name: &str) -> bool {
        self.config.auto_load.contains(&plugin_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::SqliteStore;

    #[tokio::test]
    async fn test_default_plugin_registry() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);

        let plugins = registry.load_default_plugins().await.unwrap();
        assert!(!plugins.is_empty());
        assert!(plugins.iter().any(|plugin| plugin.name() == "mail_server"));
        assert!(plugins.iter().any(|plugin| plugin.name() == "zeroclaw"));
    }

    #[tokio::test]
    async fn test_discovered_plugins_publish_schema() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);

        let plugins = registry.load_all_plugins().await.unwrap();
        let missing: Vec<String> = plugins
            .iter()
            .filter(|plugin| {
                plugin
                    .schema()
                    .map(|schema| schema.name != plugin.name())
                    .unwrap_or(true)
            })
            .map(|plugin| plugin.name().to_string())
            .collect();

        assert!(
            missing.is_empty(),
            "discovered plugins missing plugin-owned schema: {:?}",
            missing
        );
    }

    #[tokio::test]
    async fn test_loadable_plugins_publish_schema() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);
        let plugin_names = vec![
            "mcp",
            "config",
            "s6",
            "systemd",
            "dinit",
            "incus",
            "net",
            "openflow",
            "privacy_router",
            "proxmox",
            "hardware",
            "software",
            "users",
            "gcloud_adc",
            "keypair",
            "service",
            "wireguard",
            "agent_config",
            "ovsdb_bridge",
            "privacy_routes",
            "procfs",
            "rtnetlink",
            "sess_decl",
            "adc",
            "endpoint",
            "proxy_server",
            "web_ui",
        ];

        let mut missing = Vec::new();
        for plugin_name in plugin_names {
            let plugin = registry
                .load_plugin(plugin_name)
                .await
                .unwrap_or_else(|error| panic!("failed to load {}: {}", plugin_name, error));
            if plugin.schema().is_none() {
                missing.push(plugin_name.to_string());
            }
        }

        assert!(
            missing.is_empty(),
            "loadable plugins missing schema(): {:?}",
            missing
        );
    }

    #[tokio::test]
    async fn test_auto_load_config_does_not_control_registration() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());

        let config = PluginRegistryConfig {
            auto_load: vec!["s6".to_string()],
            plugin_configs: std::collections::HashMap::new(),
        };

        let registry = DefaultPluginRegistry::with_config(store, config);

        assert!(registry.is_auto_load("s6"));
        assert!(!registry.is_auto_load("mcp"));
        assert!(!registry.is_auto_load("config"));

        let plugins = registry.load_default_plugins().await.unwrap();
        assert!(plugins.iter().any(|plugin| plugin.name() == "mcp"));
        assert!(plugins.iter().any(|plugin| plugin.name() == "config"));
    }

    #[test]
    fn test_resolve_requested_plugin_name() {
        use crate::canonical;

        // Canonical path format (REQUIRED)
        assert_eq!(
            DefaultPluginRegistry::resolve_requested_plugin_name(&format!(
                "{}/procfs/memory",
                canonical::PLUGIN_BASE_PATH
            ))
            .unwrap(),
            "procfs"
        );

        // Direct plugin name
        assert_eq!(
            DefaultPluginRegistry::resolve_requested_plugin_name("systemd").unwrap(),
            "s6"
        );
        assert_eq!(
            DefaultPluginRegistry::resolve_requested_plugin_name("web-ui").unwrap(),
            "web_ui"
        );
    }

    #[tokio::test]
    async fn test_load_plugin_from_canonical_projection_path() {
        use crate::canonical;
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);

        // Use canonical path format
        let path = format!("{}/procfs/memory", canonical::PLUGIN_BASE_PATH);
        let procfs = registry.load_plugin(&path).await.unwrap();
        assert_eq!(procfs.name(), "procfs");
    }

    #[test]
    fn test_alias_paths_are_resolved() {
        assert_eq!(
            DefaultPluginRegistry::resolve_requested_plugin_name("/opdbus/v1/plugins/procfs")
                .unwrap(),
            "procfs"
        );
        assert_eq!(
            DefaultPluginRegistry::resolve_requested_plugin_name(
                "/org/opdbus/v1/plugin/plugins/procfs"
            )
            .unwrap(),
            "procfs"
        );
        assert_eq!(
            DefaultPluginRegistry::resolve_requested_plugin_name("/org/opdbus/v1/plugins/procfs")
                .unwrap(),
            "procfs"
        );
    }

    #[tokio::test]
    async fn test_unknown_plugin_auto_creates_review_draft() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);

        let plugin = registry.load_plugin("new_future_plugin").await.unwrap();
        let state = plugin.query_current_state().await.unwrap();
        assert_eq!(plugin.name(), "new_future_plugin");
        assert_eq!(
            state["pending_human_review"].as_bool(),
            Some(true),
            "unknown plugin drafts must require human review"
        );
    }

    #[tokio::test]
    async fn all_plugin_subids_are_valid_and_unique() {
        use crate::state_plugins::common::oscal::{category_required_fields, validate_subid};
        use std::collections::HashMap;

        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);
        let plugins = registry.load_all_plugins().await.unwrap();

        let mut all_subids: Vec<(String, String, String)> = Vec::new(); // plugin, key, subid
        let mut errors: Vec<String> = Vec::new();

        for plugin in &plugins {
            let name = plugin.name().to_string();
            let Some(schema) = plugin.schema() else {
                continue;
            };
            for (key, subid) in &schema.subids {
                all_subids.push((name.clone(), key.clone(), subid.clone()));
            }
        }

        // 1. Every subid must match the canonical OSCAL subid regex.
        for (plugin_name, key, subid) in &all_subids {
            if let Err(e) = validate_subid(subid) {
                errors.push(format!(
                    "plugin '{plugin_name}' key '{key}' has invalid subid '{subid}': {e}"
                ));
            }
        }

        // 2. No subid value may appear more than once across all plugins.
        let mut seen: HashMap<String, (String, String)> = HashMap::new();
        for (plugin_name, key, subid) in &all_subids {
            if let Some((prev_plugin, prev_key)) = seen.get(subid) {
                errors.push(format!(
                    "duplicate subid '{subid}' in plugin '{plugin_name}' key '{key}' (first seen in plugin '{prev_plugin}' key '{prev_key}')"
                ));
            } else {
                seen.insert(subid.clone(), (plugin_name.clone(), key.clone()));
            }
        }

        // 3. Category-specific required metadata fields must be present in the plugin schema.
        for (plugin_name, key, subid) in &all_subids {
            let Some(schema) = plugins
                .iter()
                .find(|p| p.name() == plugin_name)
                .and_then(|p| p.schema())
            else {
                continue;
            };
            let category = subid.split('.').next().unwrap_or("");
            let required_fields = category_required_fields(category);
            if category == "evt" {
                // evt.* requires at least one of event_id or event_hash.
                if !required_fields
                    .iter()
                    .any(|field| schema.fields.contains_key(*field))
                {
                    errors.push(format!(
                        "plugin '{plugin_name}' subid '{subid}' (key '{key}') is evt.* and requires one of {:?} fields, but none found in schema fields",
                        required_fields
                    ));
                }
            } else if !required_fields.is_empty() {
                for field in required_fields {
                    if !schema.fields.contains_key(*field) {
                        errors.push(format!(
                            "plugin '{plugin_name}' subid '{subid}' (key '{key}') is {category}.* and requires field '{field}', but it is not present in schema fields"
                        ));
                    }
                }
            }
        }

        assert!(
            errors.is_empty(),
            "OSCAL subid gate failures:\n{}",
            errors.join("\n")
        );
    }
}
