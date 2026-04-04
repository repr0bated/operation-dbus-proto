//! Compact MCP Server Main
//!
//! Runs the compact MCP server in stdio mode with only 4 meta-tools.

use op_mcp::compact::run_compact_stdio_server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_compact_stdio_server().await
}
