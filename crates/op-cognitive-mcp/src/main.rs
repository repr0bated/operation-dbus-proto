//! Cognitive MCP Server Binary
//!
//! Runs the cognitive memory MCP server with dynamic loading capabilities.

use clap::Parser;
use op_cognitive_mcp::CognitiveMcpServer;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "cognitive-mcp-server")]
#[command(about = "Cognitive MCP Server with memory and dynamic loading")]
struct Cli {
    /// HTTP server address
    #[arg(long, env = "COGNITIVE_MCP_BIND", default_value = "0.0.0.0:3003")]
    http: String,

    /// SQLite database path
    #[arg(
        long,
        env = "COGNITIVE_MCP_DB_PATH",
        default_value = "/var/lib/op-cognitive-mcp/memory.db"
    )]
    db: String,

    /// Log level
    #[arg(long, env = "COGNITIVE_MCP_LOG_LEVEL", default_value = "info")]
    log_level: String,
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

    info!(
        "Starting Cognitive MCP Server on {} (db: {})",
        cli.http, cli.db
    );

    let server = CognitiveMcpServer::new(&cli.db).await?;
    server.start_http_server(&cli.http).await?;

    Ok(())
}
