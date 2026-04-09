//! OP Cognitive MCP Server
//!
//! A specialized MCP server for cognitive memory and dynamic content loading.
//! Provides tools for:
//! - Memory storage and retrieval
//! - Dynamic content loading
//! - Cognitive state management
//! - Context-aware tool discovery

pub mod activity_filter;
pub mod cognitive_tools;
pub mod embedding_worker;
pub mod graph_store;
pub mod server;

pub use activity_filter::{
    derive_significance, is_pii, ActivityEvent, ActivityFilter, FilterDecision, FilterTunables,
    OpKind, Significance, SuppressReason,
};
pub use cognitive_tools::CognitiveToolRegistry;
pub use graph_store::{KnowledgeGraphStore, NamespaceNode};
pub use server::CognitiveMcpServer;
