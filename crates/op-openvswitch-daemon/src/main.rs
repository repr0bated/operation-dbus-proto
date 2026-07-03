//! op-openvswitch-daemon — Pure passthrough D-Bus proxy for rovs primitives.
//!
//! TWO separate D-Bus object paths (locked design):
//! - `/org/opdbus/rovs/jsonrpc`  → `org.opdbus.rovs.jsonrpc`
//! - `/org/opdbus/rovs/openflow` → `org.opdbus.rovs.openflow`
//!
//! The daemon is a literal passthrough: it owns the rovs crate connections
//! and exposes raw primitives over D-Bus.  Zero business logic.
//!
//! Per AGENTS.md §4: D-Bus is the ONLY control plane.

use anyhow::{Context, Result};
use std::path::Path;
use tracing::{info, warn};
use zbus::connection::Connection;

mod container;
mod dbus;
mod execution;
mod grpc;
mod grpc_streaming;
mod netns;

use dbus::{DaemonState, JsonRpcService, OpenFlowService, ProxyService};
use execution::PluginExecutionService;
use grpc_streaming::{EventBus, StreamingService};

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Args {
    grpc_addr: Option<String>,
    ovs_socket: Option<String>,
}

impl Args {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut grpc_addr = None;
        let mut ovs_socket = None;
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--grpc" => {
                    if i + 1 < args.len() {
                        grpc_addr = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--ovs-socket" => {
                    if i + 1 < args.len() {
                        ovs_socket = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }
        Self {
            grpc_addr,
            ovs_socket,
        }
    }
}

// ── Socket discovery ────────────────────────────────────────────────────────

const DEFAULT_OVS_SOCKET: &str = "/usr/local/var/run/openvswitch/db.sock";
const OVS_SOCKET_PATHS: &[&str] = &[
    "/usr/local/var/run/openvswitch/db.sock",
    "/run/openvswitch/db.sock",
    "/var/run/openvswitch/db.sock",
];

fn find_ovs_socket() -> String {
    OVS_SOCKET_PATHS
        .iter()
        .find(|p| Path::new(p).exists())
        .unwrap_or(&DEFAULT_OVS_SOCKET)
        .to_string()
}

// ── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting op-openvswitch-daemon (pure passthrough)");

    let args = Args::parse();
    let socket_path = args.ovs_socket.unwrap_or_else(find_ovs_socket);
    info!("Using OVS socket: {}", socket_path);

    let state = DaemonState::new(socket_path);

    // Eager-connect OVSDB so early callers don't wait.
    if let Err(e) = state.get_ovsdb().await {
        warn!(
            "Could not connect to OVSDB: {}. Will retry on first request.",
            e
        );
    }

    // Connect to the system D-Bus bus.
    // The projection server owns org.opdbus.v1.plugins on the system bus;
    // we request the distinct name org.opdbus.v1.plugins.ovsdb on the same
    // bus so rovs/* objects are reachable by clients dialing that name.
    let conn = Connection::system()
        .await
        .context("Failed to connect to system D-Bus")?;
    info!("Connected to system D-Bus");

    // Register TWO separate object paths (locked design).
    let grpc_state = state.clone();
    let json_service = JsonRpcService::new(state.clone());
    let of_service = OpenFlowService::new(state.clone());
    let proxy_service = ProxyService::new(state);

    conn.object_server()
        .at("/org/opdbus/rovs/jsonrpc", json_service)
        .await
        .context("Failed to register /org/opdbus/rovs/jsonrpc")?;

    conn.object_server()
        .at("/org/opdbus/rovs/openflow", of_service)
        .await
        .context("Failed to register /org/opdbus/rovs/openflow")?;

    conn.object_server()
        .at("/org/opdbus/rovs/proxy", proxy_service)
        .await
        .context("Failed to register /org/opdbus/rovs/proxy")?;

    let exec_service = PluginExecutionService::new();
    conn.object_server()
        .at("/org/opdbus/execution", exec_service)
        .await
        .context("Failed to register /org/opdbus/execution")?;

    // Request the ovsdb plugin bus name — NOT org.opdbus.v1 (owned by op-dbus-mirror).
    conn.request_name("org.opdbus.v1.plugins.ovsdb")
        .await
        .context("Failed to request bus name org.opdbus.v1.plugins.ovsdb")?;

    info!("D-Bus services registered:");
    info!("  /org/opdbus/rovs/jsonrpc  -> org.opdbus.rovs.jsonrpc");
    info!("  /org/opdbus/rovs/openflow -> org.opdbus.rovs.openflow");
    info!("  /org/opdbus/rovs/proxy    -> org.opdbus.rovs.proxy");
    info!("  /org/opdbus/execution     -> org.opdbus.execution");

    // Optional gRPC with streaming support (M2)
    if let Some(grpc_addr) = args.grpc_addr {
        let addr: std::net::SocketAddr = grpc_addr.parse().context("Invalid --grpc address")?;
        info!("Starting gRPC server on {}", addr);

        // Create shared event bus for streaming subscriptions
        let event_bus = EventBus::new();
        let streaming_service = StreamingService::new(grpc_state.clone(), event_bus);

        tokio::spawn(async move {
            if let Err(e) =
                grpc::run_grpc_server_with_streaming(addr, grpc_state, streaming_service).await
            {
                warn!("gRPC server error: {}", e);
            }
        });
    }

    info!("Daemon ready — Ctrl+C to stop");
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
