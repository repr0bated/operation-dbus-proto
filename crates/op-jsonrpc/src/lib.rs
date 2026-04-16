//! op-jsonrpc: JSON-RPC server with OVSDB and NonNet support
//!
//! This crate provides:
//! - JSON-RPC 2.0 server over Unix sockets
//! - OVSDB client for Open vSwitch integration
//! - NonNet database for non-network plugin state

pub mod nonnet;
pub mod ovsdb;
pub mod protocol;
pub mod server;

pub use nonnet::NonNetDb;
pub use ovsdb::OvsdbClient;
pub use server::JsonRpcServer;

/// Prelude for convenient imports
pub mod prelude {
    pub use super::nonnet::NonNetDb;
    pub use super::ovsdb::OvsdbClient;
    pub use super::protocol::{JsonRpcRequest, JsonRpcResponse};
    pub use super::server::JsonRpcServer;
}
