//! op-mcp-server: Unified MCP Protocol Server
//!
//! Supports multiple modes:
//!   - compact: 5 lazy meta-tools for discovering and executing system tools
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
use op_identity::{write_sled_from_wg, WireGuardIdentity};
#[cfg(feature = "grpc")]
use op_mcp::grpc::{GrpcConfig, GrpcTransport};
use op_mcp::{
    compact::LazyOpToolsExecutor,
    transport::{HttpSseTransport, StdioTransport, Transport, WebSocketTransport},
    AgentsServer, CompactServer, McpServer, McpServerConfig, ServerMode, ToolExecutor,
};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "op-mcp-server")]
#[command(about = "Unified MCP Protocol Server")]
struct Cli {
    /// Server mode: compact (5 lazy meta-tools), agents (always-on), full (all tools), grpc, grpc-agents
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

    /// Run all transports with default addresses (binds to WG interface)
    #[arg(long)]
    all: bool,

    /// WireGuard interface to read identity from
    #[arg(long, env = "WG_INTERFACE", default_value = "netmaker")]
    wg_interface: String,

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

    // ── WireGuard identity ────────────────────────────────────────────────────
    // 1. Detect local WG IP for bind address resolution.
    // 2. Write canonical IdentitySled to /dev/shm for Ghostbridge auth.
    let wg_id = WireGuardIdentity::with_interface(&cli.wg_interface);
    let wg_ip: Option<String> = wg_id.get_local_ip();

    match wg_id.get_local_pubkey() {
        Ok(pubkey) => {
            if let Err(e) = write_sled_from_wg(&pubkey) {
                tracing::warn!(error = %e, "Failed to write WG identity sled to /dev/shm");
            } else {
                info!(
                    interface = %cli.wg_interface,
                    pubkey = %pubkey,
                    wg_ip = ?wg_ip,
                    "WG identity sled written"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                interface = %cli.wg_interface,
                error = %e,
                "Could not read WG public key — identity sled not written; set WG_PUBKEY env var to override"
            );
        }
    }

    // Check for gRPC modes
    if cli.mode == "grpc" || cli.mode == "grpc-agents" {
        #[cfg(feature = "grpc")]
        {
            let port = cli
                .grpc_port
                .unwrap_or(if cli.mode == "grpc" { 50051 } else { 50052 });
            // Bind to WG interface IP if available, else 0.0.0.0.
            let bind_ip = wg_ip.as_deref().unwrap_or("0.0.0.0");
            let addr: std::net::SocketAddr = format!("{bind_ip}:{}", port).parse()?;
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

    // Determine transports.
    // When --all is used the default ports bind to the WG interface IP (or
    // 0.0.0.0 if the interface is not up). Explicit --http/--ws/--grpc flags
    // always win regardless of the WG interface state.
    let run_stdio = cli.stdio
        || cli.all
        || (cli.http.is_none() && cli.sse.is_none() && cli.ws.is_none() && cli.grpc.is_none());
    let all_ip = wg_ip.as_deref().unwrap_or("0.0.0.0");
    let http_addr = cli.http.or(cli.sse).or(if cli.all {
        Some(format!("{all_ip}:3001"))
    } else {
        None
    });
    let ws_addr = cli.ws.or(if cli.all {
        Some(format!("{all_ip}:3002"))
    } else {
        None
    });
    let grpc_addr = cli.grpc.or(if cli.all {
        Some(format!("{all_ip}:50051"))
    } else {
        None
    });

    // Create and run server based on mode
    match mode {
        ServerMode::Compact => {
            let executor: Arc<dyn ToolExecutor> = Arc::new(LazyOpToolsExecutor);
            let server = Arc::new(CompactServer::new(executor));
            info!("Compact MCP server initialized with lazy op-tools registry");

            run_transports(
                server,
                run_stdio,
                http_addr,
                ws_addr,
                grpc_addr,
                Some("/mcp/compact"),
            )
            .await
        }

        ServerMode::Full => {
            let config = McpServerConfig {
                name: cli.name,
                compact_mode: false,
                ..Default::default()
            };

            let server = McpServer::new(config).await?;
            info!(mode = %mode, "MCP server initialized");

            run_transports(server, run_stdio, http_addr, ws_addr, grpc_addr, None).await
        }

        ServerMode::Cognitive => {
            // Fan-in proxy: one authenticated caller fronting op-grpc-bridge.
            //
            // Every MCP client that instead spawns `op-cognitive-mcp --stdio` opens
            // the persistent CozoDB directly, and the second one dies on the file
            // lock. Routing through the bridge leaves a single writer, puts every
            // call through the method gate / arg validation / capability check /
            // event chain, and keeps identity in this process so clients carry no
            // credential material.
            let executor: Arc<dyn ToolExecutor> =
                Arc::new(op_mcp::cognitive_bridge::BridgeToolExecutor::connect().await?);
            let tool_count = executor.list_tools().await.map(|t| t.len()).unwrap_or(0);
            let server = Arc::new(CompactServer::new(executor));
            info!(
                tools = tool_count,
                "Cognitive fan-in MCP server initialized (sourced from op-grpc-bridge)"
            );

            run_transports(
                server,
                run_stdio,
                http_addr,
                ws_addr,
                grpc_addr,
                Some("/mcp/cognitive"),
            )
            .await
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

            run_transports(server, run_stdio, http_addr, ws_addr, grpc_addr, None).await
        }
    }
}

async fn run_transports<H>(
    server: Arc<H>,
    run_stdio: bool,
    http_addr: Option<String>,
    ws_addr: Option<String>,
    _grpc_addr: Option<String>,
    base_path: Option<&'static str>,
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
            let mut transport = HttpSseTransport::new(addr);
            if let Some(base_path) = base_path {
                transport = transport.with_base_path(base_path);
            }
            transport.serve(server).await
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
