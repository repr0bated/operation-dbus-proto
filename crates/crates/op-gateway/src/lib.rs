//! op-gateway: MCP Gateway with WireGuard authentication and smart routing

pub mod encrypted_storage;
mod error;
pub mod mcp_gateway;
pub mod wireguard_auth;

pub use encrypted_storage::*;
pub use error::*;
pub use mcp_gateway::*;
pub use wireguard_auth::*;
