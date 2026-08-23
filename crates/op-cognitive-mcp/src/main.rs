//! Cognitive MCP Server Binary
//!
//! ## DEPRECATED TRANSPORTS
//!
//! The HTTP/SSE (`:3003`) and gRPC (`:50052`) listeners are deprecated. The
//! authoritative path for every cognitive tool invocation is the bridge:
//!
//! ```text
//! org.opdbus.v1.PluginV1.Call on /org/opdbus/v1/plugins/cognitive_mcp
//! ```
//!
//! Only the bridge performs the method-existence gate, argument validation,
//! capability check and event-chain append. Calls arriving directly on `:3003`
//! or `:50052` bypass that accountability chain.
//!
//! `--no-http` / `--no-grpc` are retained for Phase 1 and will be removed once
//! consumers are migrated. `--stdio` remains for local debugging/attach.
//! See `.kiro/specs/cognitive-mcp-bridge-only-door/` and the Phase 2 spec
//! `.kiro/specs/cognitive-mcp-only-door-phase2/`.
//!
//! Transports started in parallel:
//! - HTTP/SSE  (MCP protocol, port 3003) — deprecated
//! - gRPC      (CognitiveToolService, port 50052) — deprecated
//! - D-Bus state is owned by the canonical plugin projection at
//!   org.opdbus.v1.plugins / /org/opdbus/v1/plugins/cognitive_mcp
//!
//! On startup the server reads the local WireGuard public key (from the
//! interface named by $WG_INTERFACE, defaulting to "netmaker") and writes the
//! canonical IdentitySled to /dev/shm/plugin_schema.dat so the Ghostbridge
//! interceptor and Qdrant shuttle can authenticate outbound gRPC calls.
//!
//! NOTE: on the current host no `netmaker` interface exists — netmaker is the
//! mesh (100.69.0.0/16), not an identity source, and WireGuard is terminated on
//! the upstream decoy server rather than here. The identity-sled write therefore
//! warns and no-ops. Bind addresses come from `COGNITIVE_MCP_BIND` /
//! `COGNITIVE_MCP_GRPC_BIND` in the runit run script, which point at svc0
//! (`10.200.0.2`), so no WG address promotion occurs.
//!
//! Bind address resolution order (highest priority first):
//!   1. COGNITIVE_MCP_BIND / COGNITIVE_MCP_GRPC_BIND env vars
//!   2. --http / --grpc CLI flags
//!   3. WireGuard interface IP detected at startup (if interface is up)
//!   4. 0.0.0.0 fallback

use clap::Parser;
use op_cognitive_mcp::CognitiveMcpServer;
use op_identity::WireGuardIdentity;
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

    /// CozoDB database path (defaults to :memory: to prevent direct RocksDB lock contention with op-grpc-bridge)
    #[arg(long, env = "COGNITIVE_MCP_DB_PATH", default_value = ":memory:")]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let level = match cli.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Bind configuration comes from the CLI/env only. The `/dev/shm` projection is
    // published state — what this service currently looks like to consumers — and is
    // never read back as configuration input. Reading it here previously meant a
    // stale projection could override an explicit `--no-http`/`--no-grpc`.
    // See .kiro/specs/cognitive-mcp-bridge-only-door design.md DQ-3.
    let http_enabled = !cli.no_http;
    let grpc_enabled = !cli.no_grpc;

    // ── WireGuard identity ────────────────────────────────────────────────────
    // 1. Detect local WG IP for bind address resolution.
    let wg_id = WireGuardIdentity::with_interface(&cli.wg_interface);
    let wg_ip = wg_id.get_local_ip();

    match wg_id.get_local_pubkey() {
        Ok(pubkey) => {
            info!(
                interface = %cli.wg_interface,
                pubkey = %pubkey,
                wg_ip = ?wg_ip,
                "WG identity resolved"
            );
        }
        Err(e) => {
            warn!(
                interface = %cli.wg_interface,
                error = %e,
                "Could not read WireGuard public key; set WG_PUBKEY env var to override"
            );
        }
    }

    // Resolve bind addresses: promote 0.0.0.0 defaults to WG interface IP.
    let http_addr = resolve_bind(&cli.http, wg_ip.as_deref());
    let grpc_addr = resolve_bind(&cli.grpc, wg_ip.as_deref());

    info!(
        http = %http_addr,
        grpc = %grpc_addr,
        db = %cli.db,
        wg_interface = %cli.wg_interface,
        "Starting Cognitive MCP Server"
    );

    let server = CognitiveMcpServer::new(&cli.db).await?;

    if cli.stdio {
        info!("Running stdio only (local MCP transport)");
        server.start_stdio().await?;
        return Ok(());
    }

    // Phase 1 still starts the deprecated direct listeners so existing consumers keep
    // working while they migrate to the bridge path. The `#[deprecated]` markers exist
    // to stop NEW callers being added; these call sites are the sanctioned ones and are
    // deleted wholesale in Phase 2.
    #[allow(deprecated)]
    match (!grpc_enabled, !http_enabled) {
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
