//! op-mcp-server: Unified MCP Protocol Server
//!
//! Supports multiple modes:
//!   - compact: 4 meta-tools for discovering 148+ tools (default for LLMs)
//!   - agents:  Always-on cognitive agents (memory, sequential_thinking, rust_pro, etc.)
//!   - full:    All tools directly exposed
//!   - grpc:    gRPC transport mode for high-performance internal communication
//!   - grpc-agents: gRPC transport for agents
//!
//! Supports multiple transports:
//!   op-mcp-server                           # stdio, compact mode
//!   op-mcp-server --mode agents             # stdio, agents mode
//!   op-mcp-server --http 0.0.0.0:3001       # HTTP+SSE
//!   op-mcp-server --ws 0.0.0.0:3002         # WebSocket
//!   op-mcp-server --grpc 0.0.0.0:50051      # gRPC transport
//!   op-mcp-server --all                     # All transports

use anyhow::Result;
use clap::Parser;
use op_core::BusType;
#[cfg(feature = "grpc")]
use op_mcp::grpc::{GrpcConfig, GrpcTransport};
use op_mcp::{
    transport::{HttpSseTransport, StdioTransport, Transport, WebSocketTransport},
    AgentsServer, McpServer, McpServerConfig, ServerMode,
};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "op-mcp-server")]
#[command(about = "Unified MCP Protocol Server")]
struct Cli {
    /// Server mode: compact (4 meta-tools), agents (always-on), full (all tools), grpc, grpc-agents
    #[arg(long, short, default_value = "compact")]
    mode: String,

    /// Run stdio transport (default if no network transport specified)
    #[arg(long)]
    stdio: bool,

    /// Run HTTP+SSE transport on specified address
    #[arg(long, value_name = "ADDR")]
    http: Option<String>,

    /// Run SSE-only transport on specified address
    #[arg(long, value_name = "ADDR")]
    sse: Option<String>,

    /// Run WebSocket transport on specified address
    #[arg(long, value_name = "ADDR")]
    ws: Option<String>,

    /// Run gRPC transport on specified address
    #[arg(long, value_name = "ADDR")]
    grpc: Option<String>,

    /// gRPC port (shorthand, used with --mode grpc or grpc-agents)
    #[arg(long, value_name = "PORT")]
    grpc_port: Option<u16>,

    /// Run all transports with default addresses
    #[arg(long)]
    all: bool,

    /// Disable auto-start of run-on-connection agents (agents mode only)
    #[arg(long)]
    no_auto_start: bool,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Server name override
    #[arg(long)]
    name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
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

    // Check for gRPC modes
    if cli.mode == "grpc" || cli.mode == "grpc-agents" {
        #[cfg(feature = "grpc")]
        {
            let port = cli
                .grpc_port
                .unwrap_or(if cli.mode == "grpc" { 50051 } else { 50052 });
            let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse()?;
            let server_mode = if cli.mode == "grpc-agents" {
                op_mcp::grpc::GrpcServerMode::Agents
            } else {
                op_mcp::grpc::GrpcServerMode::Compact
            };

            info!(mode = %cli.mode, port = %port, "Starting gRPC MCP server");

            let config = GrpcConfig::default()
                .with_address(addr)
                .with_mode(server_mode);

            let transport = GrpcTransport::new(config).await?;
            return transport.serve().await;
        }

        #[cfg(not(feature = "grpc"))]
        {
            anyhow::bail!("gRPC support not compiled in. Rebuild with --features grpc");
        }
    }

    // Parse server mode for non-gRPC modes
    let mode: ServerMode = cli.mode.parse().map_err(|e: String| anyhow::anyhow!(e))?;

    info!(mode = %mode, "Starting op-mcp-server");

    // Determine transports
    let run_stdio = cli.stdio
        || cli.all
        || (cli.http.is_none() && cli.sse.is_none() && cli.ws.is_none() && cli.grpc.is_none());
    let http_addr = cli.http.or(cli.sse).or(if cli.all {
        Some("0.0.0.0:3001".into())
    } else {
        None
    });
    let ws_addr = cli.ws.or(if cli.all {
        Some("0.0.0.0:3002".into())
    } else {
        None
    });
    let grpc_addr = cli.grpc.or(if cli.all {
        Some("0.0.0.0:50051".into())
    } else {
        None
    });

    // Create and run server based on mode
    match mode {
        ServerMode::Compact | ServerMode::Full => {
            let config = McpServerConfig {
                name: cli.name,
                compact_mode: mode == ServerMode::Compact,
                ..Default::default()
            };

            let server = McpServer::new(config).await?;
            info!(mode = %mode, "MCP server initialized");

            run_transports(server, run_stdio, http_addr, ws_addr, grpc_addr).await
        }

        ServerMode::Agents => {
            let bus_type = if std::env::var("DBUS_AGENT_SESSION").is_ok() {
                BusType::Session
            } else {
                BusType::System
            };

            if cli.no_auto_start {
                info!("--no-auto-start is ignored for D-Bus agents mode");
            }

            let server = Arc::new(AgentsServer::new(bus_type));
            server.initialize().await?;

            let agents = server.list_agents().await;
            let agent_ids: Vec<_> = agents.iter().map(|agent| agent.id.as_str()).collect();
            info!(
                bus = %bus_type,
                agents = ?agent_ids,
                total = agents.len(),
                "Agents MCP server initialized"
            );

            run_transports(server, run_stdio, http_addr, ws_addr, grpc_addr).await
        }
    }
}

async fn run_transports<H>(
    server: Arc<H>,
    run_stdio: bool,
    http_addr: Option<String>,
    ws_addr: Option<String>,
    _grpc_addr: Option<String>,
) -> Result<()>
where
    H: op_mcp::transport::McpHandler + 'static,
{
    let mut handles = Vec::new();

    // Spawn HTTP+SSE transport
    if let Some(addr) = http_addr {
        let server = server.clone();
        handles.push(tokio::spawn(async move {
            info!(addr = %addr, "Starting HTTP+SSE transport");
            HttpSseTransport::new(addr).serve(server).await
        }));
    }

    // Spawn WebSocket transport
    if let Some(addr) = ws_addr {
        let server = server.clone();
        handles.push(tokio::spawn(async move {
            info!(addr = %addr, "Starting WebSocket transport");
            WebSocketTransport::new(addr).serve(server).await
        }));
    }

    // gRPC transport would be spawned here if needed with the generic handler
    // For now, gRPC is handled separately with --mode grpc

    // Run stdio in main thread if enabled
    if run_stdio {
        info!("Starting stdio transport");
        StdioTransport::new().serve(server).await?;
    } else {
        for handle in handles {
            handle.await??;
        }
    }

    Ok(())
}
