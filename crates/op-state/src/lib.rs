//! op-state: State Management System
//!
//! Provides:
//! - StatePlugin trait for pluggable state management
//! - State manager for coordinating plugins
//! - Crypto utilities for state hashing/signing
//! - Schema validation
//! - Plugin tree for hierarchical state
//! - Persistent storage via op-state-store
//! - Auto-plugin generation

pub mod authority;
// pub mod auto_plugin;
pub mod crypto;
pub mod dbus_plugin_base;
pub mod dbus_server;
pub mod manager;
pub mod plugin;
pub mod plugin_workflow;
pub mod plugtree;
pub mod schema_validator;

pub use manager::{FootprintSender, StateManager};
pub use plugin::{
    ApplyResult, ChangeOperation, Checkpoint, DesiredState, DiffMetadata, PluginCapabilities,
    StateAction, StateChange, StateDiff, StatePlugin, StateSource, ValidationError,
    ValidationResult,
};
pub use plugtree::PlugTree;

// Re-export state store types
pub use op_state_store::{
    ExecutionJob, ExecutionResult, ExecutionStatus, PluginSchema, SchemaRegistry, SqliteStore,
    StateStore, StateStoreError,
};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::manager::StateManager;
    pub use super::plugin::{
        ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff,
        StatePlugin,
    };
    pub use super::plugtree::PlugTree;
    // State store types
    pub use op_state_store::{
        ExecutionJob, ExecutionStatus, PluginSchema, SchemaRegistry, SqliteStore, StateStore,
    };
}
