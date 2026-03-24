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

pub mod auto_create;
pub mod builtin;
pub mod chat;
pub mod dynamic_loading;
pub mod plugin;
pub mod registry;
pub mod service_def;
pub mod state;

// State plugins - each manages a specific domain
pub mod default_registry;
pub mod state_plugins;

pub use auto_create::AutoPlugin;
pub use default_registry::{DefaultPluginRegistry, PluginRegistryConfig};
pub use plugin::{Plugin, PluginCapabilities, PluginContext, PluginMetadata};
pub use registry::{PluginRecord, PluginRegistry};
pub use state::{ChangeOperation, DesiredState, StateChange, ValidationResult};

// Re-export chat types
pub use chat::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, ExecutionStatus, TokenUsage, ToolCall,
};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::auto_create::AutoPlugin;
    pub use super::registry::PluginRegistry;
    pub use super::state::{ChangeOperation, DesiredState, StateChange, ValidationResult};

    // Re-export state plugins
    pub use super::dynamic_loading::DynamicLoadingPlugin;
    pub use super::state_plugins::*;
}
