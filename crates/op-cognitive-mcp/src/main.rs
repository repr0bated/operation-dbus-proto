//! Cognitive MCP Server Binary
//!
//! Transports started in parallel:
//! - HTTP/SSE  (MCP protocol, port 3003)
//! - gRPC      (CognitiveToolService, port 50052)
//! - D-Bus     (org.opdbus.CognitiveMcp / /org/opdbus/v1/cognitive)

use clap::Parser;
use op_cognitive_mcp::CognitiveMcpServer;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "cognitive-mcp-server")]
#[command(about = "Cognitive MCP Server with memory, NotebookLM bridge, gRPC, and D-Bus")]
struct Cli {
    /// HTTP/SSE server address (MCP protocol)
    #[arg(long, env = "COGNITIVE_MCP_BIND", default_value = "0.0.0.0:3003")]
    http: String,

    /// gRPC server address (CognitiveToolService)
    #[arg(long, env = "COGNITIVE_MCP_GRPC_BIND", default_value = "0.0.0.0:50052")]
    grpc: String,

    /// CozoDB database path
    #[arg(
        long,
        env = "COGNITIVE_MCP_DB_PATH",
        default_value = "/var/lib/op-cognitive-mcp/memory.db"
    )]
    db: String,

    /// Log level
    #[arg(long, env = "COGNITIVE_MCP_LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Disable gRPC server
    #[arg(long, env = "COGNITIVE_MCP_GRPC_DISABLED")]
    no_grpc: bool,

    /// Disable HTTP/SSE server
    #[arg(long, env = "COGNITIVE_MCP_HTTP_DISABLED")]
    no_http: bool,

    /// Disable D-Bus registration
    #[arg(long, env = "COGNITIVE_MCP_DBUS_DISABLED")]
    no_dbus: bool,
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
        http = %cli.http,
        grpc = %cli.grpc,
        db = %cli.db,
        "Starting Cognitive MCP Server"
    );

    let server = CognitiveMcpServer::new(&cli.db).await?;

    // D-Bus: start first, keep connection alive for the process lifetime.
    let _dbus_conn = if !cli.no_dbus {
        match server.start_dbus().await {
            Ok(conn) => {
                info!("D-Bus registered: org.opdbus.CognitiveMcp");
                Some(conn)
            }
            Err(e) => {
                warn!("D-Bus registration failed (continuing without it): {e}");
                None
            }
        }
    } else {
        None
    };

    match (cli.no_grpc, cli.no_http) {
        (true, true) => {
            eprintln!("Error: both --no-grpc and --no-http specified. Nothing to run.");
            std::process::exit(1);
        }
        (true, false) => {
            info!("Running HTTP/SSE only");
            server.start_http_server(&cli.http).await?;
        }
        (false, true) => {
            info!("Running gRPC only");
            server.start_grpc_server(&cli.grpc).await?;
        }
        (false, false) => {
            info!("Running HTTP/SSE + gRPC");
            server.start_dual(&cli.http, &cli.grpc).await?;
        }
    }

    Ok(())
}
