//! Default plugin loader - auto-loads essential plugins
//!
//! This module defines which plugins are loaded by default when the system starts.
//! Plugins can be enabled/disabled via configuration.

use anyhow::{anyhow, Result};
use op_state_store::StateStore;
use simd_json::prelude::*;
use std::sync::Arc;

use crate::state_plugins::{
    AdcPlugin, AgentConfigPlugin, CognitiveMcpPlugin, CompactMcpPlugin, ConfigPlugin,
    CtlPlaneChatbotPlugin, EndpointPlugin, GcloudAdcPlugin, HardwarePlugin, IncusPlugin,
    KeypairPlugin, MailServerPlugin, McpStatePlugin, NetStatePlugin, OpenFlowPlugin,
    OvsBridgePlugin, PrivacyRouterPlugin, PrivacyRoutesPlugin, ProcfsPlugin, ProxmoxPlugin,
    ProxyServerPlugin, RtnetlinkPlugin, S6StatePlugin, ServicePlugin, SessDeclPlugin,
    SoftwarePlugin, UnixSocketPlugin, UsersPlugin, WebUiPlugin, WireGuardPlugin, ZeroclawPlugin,
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
        "net".to_string(),
        "openflow".to_string(),
        "ovsdb_bridge".to_string(),
        "privacy_router".to_string(),
        "privacy_routes".to_string(),
        "procfs".to_string(),
        "rtnetlink".to_string(),
        "agent_config".to_string(),
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
}

impl DefaultPluginRegistry {
    /// Create a new plugin loader
    pub fn new(state_store: Arc<dyn StateStore>) -> Self {
        Self {
            config: PluginRegistryConfig::default(),
            state_store,
        }
    }

    /// Create with custom configuration
    pub fn with_config(state_store: Arc<dyn StateStore>, config: PluginRegistryConfig) -> Self {
        Self {
            config,
            state_store,
        }
    }

    /// Resolve user/request-facing plugin references into canonical loader names.
    ///
    /// Supports direct names, aliases, and projection paths like
    /// `/opdbus/v1/plugins/<plugin>/...`.
    pub fn resolve_requested_plugin_name(requested: &str) -> Result<String> {
        let trimmed = requested.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("Plugin identifier cannot be empty"));
        }

        let from_path = Self::extract_plugin_name_from_projection_path(trimmed).unwrap_or(trimmed);
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

    fn extract_plugin_name_from_projection_path(requested: &str) -> Option<&str> {
        const PREFIXES: [&str; 2] = ["/opdbus/v1/plugins/", "/org/opdbus/v1/plugins/"];

        for prefix in PREFIXES {
            if let Some(rest) = requested.strip_prefix(prefix) {
                return rest.split('/').find(|segment| !segment.is_empty());
            }
        }

        None
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
            "config" => {
                let config_path =
                    self.get_plugin_config_path("config", "/etc/op-dbus/config-store.json");
                Arc::new(ConfigPlugin::new(config_path))
            }
            "cognitive_mcp" => Arc::new(CognitiveMcpPlugin::new()),
            "compact_mcp" => Arc::new(CompactMcpPlugin::new()),
            "ctl_plane_chatbot" => Arc::new(CtlPlaneChatbotPlugin::new()),
            "s6" => Arc::new(S6StatePlugin::new()),
            "incus" => Arc::new(IncusPlugin::new()),
            "mail_server" => Arc::new(MailServerPlugin::new()),
            "unix_socket" => Arc::new(UnixSocketPlugin::new()),
            "net" => Arc::new(NetStatePlugin::new()),
            "openflow" => Arc::new(OpenFlowPlugin::new()),
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
            "privacy_routes" => Arc::new(PrivacyRoutesPlugin::default()),
            "procfs" => Arc::new(ProcfsPlugin::new()),
            "rtnetlink" => Arc::new(RtnetlinkPlugin::new()),
            "sess_decl" => Arc::new(SessDeclPlugin::new()),
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

    /// Get list of available plugins
    pub fn available_plugins() -> Vec<&'static str> {
        vec![
            "mcp",
            "zeroclaw",
            "cognitive_mcp",
            "compact_mcp",
            "ctl_plane_chatbot",
            "config",
            "s6",
            "incus",
            "net",
            "privacy_routes",
            "openflow",
            "privacy_router",
            // "netmaker",
            // "lxc",
            // "packagekit",
        ]
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
        assert_eq!(
            DefaultPluginRegistry::resolve_requested_plugin_name(
                "/opdbus/v1/plugins/procfs/memory"
            )
            .unwrap(),
            "procfs"
        );
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
    async fn test_load_plugin_from_projection_path() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);

        let procfs = registry
            .load_plugin("/opdbus/v1/plugins/procfs/memory")
            .await
            .unwrap();
        assert_eq!(procfs.name(), "procfs");
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
