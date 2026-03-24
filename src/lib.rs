//! OP-DBUS: Native, Deterministic Control Plane for Linux Systems
//!
//! Core invariants:
//! - JSON-RPC is the sole execution interface
//! - Tools are the only mutation mechanism
//! - Database state is authoritative reality
//! - D-Bus is a read-only projection
//! - Chatbot reasons but never executes directly
//! - Inspector Gadget is one-shot only (discovery, migration, schema)
//! - MCP is ingress only, never an execution engine
//! - Antigravity tunnel is development-only

pub mod json_rpc;
// pub mod execution;
// pub mod tool;
pub mod plugin;
pub mod work_stack;
// pub mod orchestrator;
pub mod blockchain;
pub mod cache;
pub mod mcp;
pub mod mcp_live;
// pub mod state_store;
// pub mod dbus_projection;
// pub mod projection;
pub mod chatbot;
pub mod error;
pub mod plugins;
pub mod pre_canned;

// Core system components
pub mod inspector_gadget;
pub mod policy;
pub mod dependency;
pub mod disaster_recovery;
pub mod vectorization;
pub mod numa_cache;
pub mod security;

// Web interface
#[cfg(feature = "web")]
pub mod web;

// Development-only antigravity tunnel
#[cfg(feature = "dev-antigravity")]
pub mod antigravity;

// Re-exports
pub use json_rpc::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};
pub use op_execution_tracker::{
    hash_execution, ExecutionRecord, ExecutionStats, ExecutionTiming, ExecutionTracker,
    RecordExecutionStatus as ExecutionStatus,
};
pub use op_tools::{Tool, ToolRegistry};
pub use op_core::{ToolResult, ToolRequest as ToolContext}; // Adjusted names for compatibility
pub use op_core::types::BusType;
pub use crate::plugin::{MirrorState, StateChange, ValidationResult, StateSource, ChangeOperation, ValidationError};
pub use op_plugins::registry::PluginRegistry;
pub use op_plugins::plugin::{PluginMetadata as PluginCore, BoxedPlugin as EffectivePlugin}; // Adjusted for naming compatibility
pub use work_stack::{WorkStack, WorkStackNode, WorkStackExecution, VectorClock};
pub use op_workflows::orchestrator::Orchestrator;
pub use blockchain::{BlockchainStream, ChainBlock};
pub use cache::BtrfsCache;
pub use mcp::McpCompactDispatcher;
pub use mcp_live::{McpLiveDispatcher, LiveAgent, CognitiveStream};
pub use op_state_store::StateStore;
pub use op_introspection::projection::DbusProjection;
pub mod projection {
    pub use op_tools::discovery::projection_engine::ProjectionEngine;
}
pub use chatbot::{Chatbot, ChatSession, ChatMessage, ChatResponse};
pub use inspector_gadget::{InspectorGadget, DiscoveryResult, MigrationPlan};
pub use policy::{PolicyEngine, Policy, ComplianceProfile};
pub use dependency::{DependencyManager, PackageDependency, ServiceDependency};
pub use disaster_recovery::{DisasterRecovery, CanonicalExport};
pub use error::{OpDbusError, Result};
pub use security::{SecurityValidator, ToolSecurityProfile, AccessLevel, SecurityError};

#[cfg(feature = "dev-antigravity")]
pub use antigravity::{AntigravityTunnel, DevTunnelRequest, DevSession};

/// System-wide constants
pub mod constants {
    pub const JSONRPC_VERSION: &str = "2.0";
    pub const WORK_STACK_PROMOTION_THRESHOLD: u64 = 25;
    pub const BTRFS_CACHE_SUBVOL_PREFIX: &str = "/var/lib/op-dbus/cache";
    pub const STATE_DB_PATH: &str = "/var/lib/op-dbus/state.db";
    pub const DBUS_NAMESPACE: &str = "org.opdbus";
    pub const WEB_DEFAULT_PORT: u16 = 8080;
    pub const CHATBOT_MAX_HISTORY: usize = 100;
    pub const MODEL_DIR: &str = "/var/lib/op-dbus/models";
    
    #[cfg(feature = "dev-antigravity")]
    pub const ANTIGRAVITY_DEFAULT_PORT: u16 = 9999;
    #[cfg(feature = "dev-antigravity")]
    pub const ANTIGRAVITY_SESSION_TIMEOUT_SECS: u64 = 3600;
}
