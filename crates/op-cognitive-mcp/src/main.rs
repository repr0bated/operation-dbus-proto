//! Cognitive MCP Server Binary
//!
//! Post-Phase 2: stdio is the only transport. Tool execution for the bridge
//! accountability path happens in-process in `op-grpc-bridge`. This binary
//! remains for local MCP attach and CozoDB-backed stdio clients.
//!
//! See `.kiro/specs/cognitive-mcp-only-door-phase2/`.

use clap::Parser;
use op_cognitive_mcp::CognitiveMcpServer;

use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "cognitive-mcp-server")]
#[command(about = "Cognitive MCP Server (stdio transport)")]
struct Cli {
    /// CozoDB database path
    #[arg(
        long,
        env = "COGNITIVE_MCP_DB_PATH",
        default_value = "/var/lib/op-cognitive-mcp/memory.db"
    )]
    db: String,

    /// WireGuard interface to read identity from (Qdrant shuttle / outbound gRPC)
    #[arg(long, env = "WG_INTERFACE", default_value = "netmaker")]
    wg_interface: String,

    /// Log level
    #[arg(long, env = "COGNITIVE_MCP_LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Stdio transport (only mode; retained for runit / MCP client compatibility)
    #[arg(long, env = "COGNITIVE_MCP_STDIO")]
    stdio: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let level = match cli.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Identity is no longer projected through the legacy 152-byte sled at
    // `/dev/shm/plugin_schema.dat`. The session genesis is minted at arrival
    // by the MutationEngine; this process only needs its own WireGuard pubkey
    // for outbound auth, not a shared last-write-wins file every process
    // raced to overwrite.

    info!(db = %cli.db, wg_interface = %cli.wg_interface, "Starting Cognitive MCP Server");

    let server = CognitiveMcpServer::new(&cli.db).await?;

    // Post-Phase 2: stdio is the only transport.
    // Tool execution happens in-process in op-grpc-bridge.
    let _ = cli.stdio;
    info!("Running stdio only (in-process registry is in op-grpc-bridge)");
    server.start_stdio().await?;

    Ok(())
}
