#![recursion_limit = "512"]

//! op-plugins: Plugin system with state management and blockchain footprints
//!
//! Features:
//! - Plugin trait with desired state management
//! - State plugins for network, LXC, systemd, OpenFlow, etc.
//! - BTRFS subvolume storage per plugin
//! - Automatic hash footprints for blockchain audit trail
//! - Auto-creation of missing plugins
//! - Lifecycle hooks
//! - Canonical plugin-document persistence into the schema catalog

pub mod auto_create;
pub mod builtin;
pub mod canonical;
pub mod chat;
pub mod dynamic_loading;
pub mod plugin;
pub mod registry;
pub mod schema_loader;
pub mod service_def;
pub mod state;

// State plugins - each manages a specific domain
pub mod default_registry;
pub mod state_plugins;

pub use auto_create::AutoPlugin;
pub use canonical as plugin_paths;
pub use default_registry::{DefaultPluginRegistry, PluginRegistryConfig};
pub use plugin::{Plugin, PluginCapabilities, PluginContext, PluginMetadata};
pub use registry::PluginRegistry as PluginCatalog;
pub use registry::{PluginRecord, PluginRegistry};
pub use schema_loader::SchemaLoader;
pub use state::{ChangeOperation, DesiredState, StateChange, ValidationResult};

// Re-export chat types
pub use chat::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, ExecutionStatus, TokenUsage, ToolCall,
};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::auto_create::AutoPlugin;
    pub use super::registry::PluginRegistry as PluginCatalog;
    pub use super::registry::PluginRegistry;
    pub use super::state::{ChangeOperation, DesiredState, StateChange, ValidationResult};

    // Re-export state plugins
    pub use super::dynamic_loading::DynamicLoadingPlugin;
    pub use super::state_plugins::{
        AdcPlugin, AgentConfigPlugin, AntigravityChatPlugin, AntigravityPlugin, BtrfsPlugin,
        CognitiveMcpPlugin, CompactMcpPlugin, ConfigPlugin, CronPlugin, CtlPlaneChatbotPlugin,
        DnsResolverPlugin, EndpointPlugin, ExecutionResult, FactoryPlugin, Fail2banPlugin,
        FreeDesktopPlugin, FullSystemPlugin, GcloudAdcPlugin, HardwarePlugin, IncusPlugin,
        KeypairPlugin, KeyringPlugin, KnowledgePlugin, Login1Plugin, LxcPlugin, MailServerPlugin,
        McpStatePlugin, MemoryPlugin, NetStatePlugin, NetmakerConfig, NetmakerPlugin,
        OpenFlowObfuscationPlugin, OpenFlowPlugin, OvsBridgePlugin, OvsdbDaemonPlugin,
        PackageKitPlugin, PciDeclPlugin, PrivacyRouterPlugin, PrivacyRoutesPlugin, ProcfsPlugin,
        ProxmoxPlugin, ProxyServerPlugin, RovsCommandsPlugin, RtnetlinkPlugin, S6StatePlugin,
        SchemaRendererPlugin, ServicePlugin, SessDeclPlugin, SoftwarePlugin, ToolDefinition,
        UnixSocketPlugin, UsersPlugin, WebUiPlugin, WireGuardPlugin, WorkflowsPlugin,
        ZeroclawPlugin,
    };
}
pub mod state_publisher;
