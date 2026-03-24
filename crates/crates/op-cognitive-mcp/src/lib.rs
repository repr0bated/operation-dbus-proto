//! OP Cognitive MCP Server
//!
//! A specialized MCP server for cognitive memory and dynamic content loading.
//! Provides tools for:
//! - Memory storage and retrieval
//! - Dynamic content loading
//! - Cognitive state management
//! - Context-aware tool discovery

pub mod memory_store;
pub mod cognitive_tools;
pub mod dynamic_loader;
pub mod server;

pub use server::CognitiveMcpServer;
pub use memory_store::CognitiveMemoryStore;
pub use cognitive_tools::CognitiveToolRegistry;