//! Default plugin loader - auto-loads essential plugins
//!
//! This module defines which plugins are loaded by default when the system starts.
//! Plugins can be enabled/disabled via configuration.
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
    KnowledgePlugin, Login1Plugin, MailServerPlugin, McpStatePlugin, MemoryPlugin,
    NetStatePlugin, NetmakerConfig, NetmakerPlugin, OpenFlowPlugin, OvsBridgePlugin,
    OvsdbDaemonPlugin, PackageKitPlugin, PciDeclPlugin, ProcfsPlugin, ProxyServerPlugin,
    RovsCommandsPlugin, RtnetlinkPlugin, S6StatePlugin, SchemaRendererPlugin, ServicePlugin,
    SessDeclPlugin, SoftwarePlugin, UnixSocketPlugin, UsersPlugin, WebUiPlugin, WgcfPlugin,
    WireGuardPlugin, WorkflowsPlugin, XrayPlugin, ZeroclawPlugin,
};
use crate::AutoPlugin;

/// Default plugin loader configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginRegistryConfig {
    /// Auto-load plugins on startup
    #[serde(default = "default_auto_load")]
    pub auto_load: Vec<String>,

    /// Plugin-specific configurations
    #[serde(default)]
    pub plugin_configs: std::collections::HashMap<String, simd_json::OwnedValue>,
}

fn default_auto_load() -> Vec<String> {
    let wg_only = std::env::var("OP_DBUS_WG_ONLY")
        .ok()
        .map(|v| {
            let l = v.to_lowercase();
            !(l == "0" || l == "false" || l == "no")
        })
        .unwrap_or(false);

    if wg_only {
        return vec![
            "zeroclaw".to_string(),
            "config".to_string(),
            "service".to_string(),
            "s6".to_string(),
            "net".to_string(),
            "rtnetlink".to_string(),
            "procfs".to_string(),
            "wireguard".to_string(),
            "agent_config".to_string(),
        ];
    }

    vec![
        "mcp".to_string(),
        "zeroclaw".to_string(),
        "cognitive_mcp".to_string(),
        "compact_mcp".to_string(),
        "config".to_string(),
        "s6".to_string(),
        "incus".to_string(),
        "mail_server".to_string(),
        "unix_socket".to_string(),
        "wgcf".to_string(),
        "xray".to_string(),
        "net".to_string(),
        "openflow".to_string(),
        "ovsdb_bridge".to_string(),
        "ovsdb_daemon".to_string(),
        "rovs_commands".to_string(),
        "procfs".to_string(),
        "rtnetlink".to_string(),
        "agent_config".to_string(),
        // Always-loaded knowledge / compliance / schema plugins
        "memory".to_string(),
        "knowledge".to_string(),
        "schema_renderer".to_string(),
        "workflows".to_string(),
        "netmaker".to_string(),
    ]
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

    /// Load all auto-load plugins
    pub async fn load_default_plugins(&self) -> Result<Vec<Arc<dyn op_state::StatePlugin>>> {
        let mut plugins: Vec<Arc<dyn op_state::StatePlugin>> = Vec::new();

        for plugin_name in &self.config.auto_load {
            match self.load_plugin(plugin_name).await {
                Ok(plugin) => {
                    if !plugin.is_available() {
                        tracing::info!(
                            "Skipping unavailable plugin {}: {}",
                            plugin_name,
                            plugin.unavailable_reason()
                        );
                        continue;
                    }
                    tracing::info!("✅ Loaded plugin: {}", plugin_name);
                    plugins.push(plugin);
                }
                Err(e) => {
                    tracing::warn!("⚠️ Failed to load plugin {}: {}", plugin_name, e);
                }
            }
        }

        tracing::info!("📦 Loaded {} plugins", plugins.len());
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
            "procfs" => Arc::new(ProcfsPlugin::new()),
            "rovs_commands" => Arc::new(RovsCommandsPlugin::new()),
            "rtnetlink" => Arc::new(RtnetlinkPlugin::new()),
            "sess_decl" => Arc::new(SessDeclPlugin::new()),
            "netmaker" => Arc::new(NetmakerPlugin::new(NetmakerConfig::default())),
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

    /// Get list of available plugins (built-in list)
    pub fn available_plugins() -> Vec<&'static str> {
        vec![
            "mcp",
            "zeroclaw",
            "freedesktop",
            "cognitive_mcp",
            "compact_mcp",
            "ctl_plane_chatbot",
            "config",
            "s6",
            "incus",
            "net",
            "wireguard",
            "web_ui",
            "openflow",
            // "netmaker",
            // "packagekit",
        ]
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

        // Check default auto-load plugins
        assert!(registry.is_auto_load("mcp"));
        assert!(registry.is_auto_load("config"));
        assert!(registry.is_auto_load("s6"));
        assert!(registry.is_auto_load("net"));

        // Load plugins
        let plugins = registry.load_default_plugins().await.unwrap();
        assert!(!plugins.is_empty());
    }

    #[tokio::test]
    async fn test_auto_loaded_plugins_publish_schema() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);

        let plugins = registry.load_default_plugins().await.unwrap();
        let missing: Vec<String> = plugins
            .iter()
            .filter(|plugin| plugin.schema().is_none())
            .map(|plugin| plugin.name().to_string())
            .collect();

        assert!(
            missing.is_empty(),
            "auto-loaded plugins missing schema(): {:?}",
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
            "hardware",
            "software",
            "users",
            "gcloud_adc",
            "keypair",
            "service",
            "wireguard",
            "agent_config",
            "ovsdb_bridge",
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
    async fn test_custom_config() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());

        let config = PluginRegistryConfig {
            auto_load: vec!["s6".to_string()],
            plugin_configs: std::collections::HashMap::new(),
        };

        let registry = DefaultPluginRegistry::with_config(store, config);

        assert!(registry.is_auto_load("s6"));
        assert!(!registry.is_auto_load("mcp"));
        assert!(!registry.is_auto_load("config"));
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
}
