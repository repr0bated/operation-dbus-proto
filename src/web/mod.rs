//! Web Interface Module
//!
//! Provides HTTP/WebSocket interface for the MCP server.

mod server;
mod routes;
mod websocket;

pub use server::{WebServer, WebServerConfig};
pub use routes::create_router;
pub use websocket::WebSocketHandler;
