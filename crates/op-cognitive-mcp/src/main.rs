//! Cognitive MCP Server Binary
//!
//! Transports started in parallel:
//! - HTTP/SSE  (MCP protocol, port 3003)
//! - gRPC      (CognitiveToolService, port 50052)
//! - D-Bus state is owned by the canonical plugin projection at
//!   org.opdbus.v1.plugins / /org/opdbus/v1/plugins/cognitive_mcp
//!
//! On startup the server reads the local WireGuard public key (from the
//! interface named by $WG_INTERFACE, defaulting to "netmaker") and writes the
//! canonical IdentitySled to /dev/shm/plugin_schema.dat so the Ghostbridge
//! interceptor and Qdrant shuttle can authenticate outbound gRPC calls.
//!
//! Bind address resolution order (highest priority first):
//!   1. COGNITIVE_MCP_BIND / COGNITIVE_MCP_GRPC_BIND env vars
//!   2. --http / --grpc CLI flags
//!   3. WireGuard interface IP detected at startup (if interface is up)
//!   4. 0.0.0.0 fallback

use clap::Parser;
use op_cognitive_mcp::CognitiveMcpServer;
use op_identity::{write_sled_from_wg, WireGuardIdentity};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "cognitive-mcp-server")]
#[command(about = "Cognitive MCP Server with memory, NotebookLM bridge, HTTP, and gRPC")]
struct Cli {
    /// HTTP/SSE server address (MCP protocol).
    /// If left at 0.0.0.0 the WireGuard interface IP is used when available.
    /// Override with COGNITIVE_MCP_BIND env var or this flag.
    #[arg(long, env = "COGNITIVE_MCP_BIND", default_value = "0.0.0.0:3003")]
    http: String,

    /// gRPC server address (CognitiveToolService).
    /// If left at 0.0.0.0 the WireGuard interface IP is used when available.
    /// Override with COGNITIVE_MCP_GRPC_BIND env var or this flag.
    #[arg(long, env = "COGNITIVE_MCP_GRPC_BIND", default_value = "0.0.0.0:50052")]
    grpc: String,

    /// CozoDB database path
    #[arg(
        long,
        env = "COGNITIVE_MCP_DB_PATH",
        default_value = "/var/lib/op-cognitive-mcp/memory.db"
    )]
    db: String,

    /// WireGuard interface to read identity from
    #[arg(long, env = "WG_INTERFACE", default_value = "netmaker")]
    wg_interface: String,

    /// Log level
    #[arg(long, env = "COGNITIVE_MCP_LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Disable gRPC server
    #[arg(long, env = "COGNITIVE_MCP_GRPC_DISABLED")]
    no_grpc: bool,

    /// Disable HTTP/SSE server
    #[arg(long, env = "COGNITIVE_MCP_HTTP_DISABLED")]
    no_http: bool,

    /// Run stdio transport only (for local MCP clients — direct, no network)
    #[arg(long, env = "COGNITIVE_MCP_STDIO")]
    stdio: bool,
}

/// Promote an `0.0.0.0:PORT` default address to `<wg_ip>:PORT` when the WG
/// interface is up.  Explicit addresses (not starting with `0.0.0.0:`) are
/// returned unchanged so env-var or flag overrides always win.
fn resolve_bind(addr: &str, wg_ip: Option<&str>) -> String {
    if let Some(rest) = addr.strip_prefix("0.0.0.0:") {
        if let Some(ip) = wg_ip {
            return format!("{ip}:{rest}");
        }
    }
    addr.to_string()
}

/// Resolve bind config (http/grpc addresses, wg_interface) from the
/// `cognitive_mcp` plugin's live `/dev/shm` projection when present. Falls
/// back to the already-parsed CLI/env values unchanged, so a cold start
/// (before the projection is first seeded by op-grpc-bridge) behaves exactly
/// as it did before this wiring existed.
///
/// OSCAL subid: obs.service.cognitive-mcp.bind-config.resolve@v1
fn cognitive_mcp_bind_config(
    cli: &Cli,
) -> op_plugins::state_plugins::cognitive_mcp::CognitiveMcpConfig {
    use op_plugins::state_plugins::cognitive_mcp::CognitiveMcpConfig;

    op_core::projection_shm::read_projection_bytes("cognitive_mcp")
        .and_then(|bytes| serde_json::from_slice::<CognitiveMcpConfig>(&bytes).ok())
        .unwrap_or_else(|| CognitiveMcpConfig {
            http: cli.http.clone(),
            grpc: cli.grpc.clone(),
            wg_interface: cli.wg_interface.clone(),
            http_enabled: !cli.no_http,
            grpc_enabled: !cli.no_grpc,
            dbus_enabled: true,
        })
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

    // Resolve bind config from the cognitive_mcp plugin's live projection when
    // present (op-grpc-bridge seeds/updates it); falls back to the CLI/env
    // values above, unchanged, when the projection is absent (cold start).
    let bind_config = cognitive_mcp_bind_config(&cli);

    // ── WireGuard identity ────────────────────────────────────────────────────
    // 1. Detect local WG IP for bind address resolution.
    // 2. Write canonical IdentitySled to /dev/shm for Ghostbridge auth.
    let wg_id = WireGuardIdentity::with_interface(&bind_config.wg_interface);
    let wg_ip = wg_id.get_local_ip();

    match wg_id.get_local_pubkey() {
        Ok(pubkey) => {
            info!(
                interface = %bind_config.wg_interface,
                pubkey = %pubkey,
                wg_ip = ?wg_ip,
                "Writing WireGuard identity sled to /dev/shm/plugin_schema.dat"
            );
            if let Err(e) = write_sled_from_wg(&pubkey) {
                warn!(
                    error = %e,
                    "Failed to write identity sled — gRPC Ghostbridge auth will not work"
                );
            }
        }
        Err(e) => {
            warn!(
                interface = %bind_config.wg_interface,
                error = %e,
                "Could not read WireGuard public key — identity sled not written; \
                 set WG_PUBKEY env var to override"
            );
        }
    }

    // Resolve bind addresses: promote 0.0.0.0 defaults to WG interface IP.
    let http_addr = resolve_bind(&bind_config.http, wg_ip.as_deref());
    let grpc_addr = resolve_bind(&bind_config.grpc, wg_ip.as_deref());

    info!(
        http = %http_addr,
        grpc = %grpc_addr,
        db = %cli.db,
        wg_interface = %bind_config.wg_interface,
        "Starting Cognitive MCP Server"
    );

    let server = CognitiveMcpServer::new(&cli.db).await?;

    if cli.stdio {
        info!("Running stdio only (local MCP transport)");
        server.start_stdio().await?;
        return Ok(());
    }

    match (!bind_config.grpc_enabled, !bind_config.http_enabled) {
        (true, true) => {
            eprintln!("Error: both gRPC and HTTP transports are disabled. Nothing to run.");
            std::process::exit(1);
        }
        (true, false) => {
            info!("Running HTTP/SSE only");
            server.start_http_server(&http_addr).await?;
        }
        (false, true) => {
            info!("Running gRPC only");
            server.start_grpc_server(&grpc_addr).await?;
        }
        (false, false) => {
            info!("Running HTTP/SSE + gRPC");
            server.start_dual(&http_addr, &grpc_addr).await?;
        }
    }

    Ok(())
}
