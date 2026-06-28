//! op-openvswitch-daemon — OVSDB D-Bus passthrough
//!
//! D-Bus object path:
//! - `/org/opdbus/v1/plugins/ovsdb` → `org.opdbus.v1.plugins.ovsdb`
//!
//! The daemon owns the OVSDB connection and exposes JSON-RPC primitives over D-Bus.
//! Per AGENTS.md §4: D-Bus is the ONLY control plane.

use anyhow::{Context, Result};
use std::path::Path;
use tracing::{info, warn};
use zbus::connection::Connection;

mod dbus;
mod grpc;
mod grpc_streaming;
mod netns;

use dbus::{DaemonState, JsonRpcService};
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

const DEFAULT_OVS_SOCKET: &str = "/var/run/openvswitch/db.sock";
const OVS_SOCKET_PATHS: &[&str] = &["/run/openvswitch/db.sock", "/var/run/openvswitch/db.sock"];

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

    info!("Starting op-openvswitch-daemon (OVSDB passthrough)");

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

    let conn = Connection::system()
        .await
        .context("Failed to connect to system D-Bus")?;
    info!("Connected to system D-Bus");

    // Register D-Bus object at /org/opdbus/v1/plugins/ovsdb on our own bus name.
    // We use org.opdbus.v1.plugins.ovsdb (not the bare org.opdbus.v1).
    // Note: The bridge owns org.opdbus.v1 and /org/opdbus/v1/plugins/* - this daemon
    // provides passthrough to the local OVSDB socket.
    conn.request_name("org.opdbus.v1.plugins.ovsdb")
        .await
        .context("Failed to request bus name org.opdbus.v1.plugins.ovsdb")?;

    let json_service = JsonRpcService::new(state.clone());
    conn.object_server()
        .at("/org/opdbus/v1/plugins/ovsdb", json_service)
        .await
        .context("Failed to register /org/opdbus/v1/plugins/ovsdb")?;

    info!("D-Bus service registered:");
    info!("  /org/opdbus/v1/plugins/ovsdb on org.opdbus.v1.plugins.ovsdb");

    // Optional gRPC with streaming support (M2)
    if let Some(grpc_addr) = args.grpc_addr {
        let addr: std::net::SocketAddr = grpc_addr.parse().context("Invalid --grpc address")?;
        info!("Starting gRPC server on {}", addr);

        // Create shared event bus for streaming subscriptions
        let event_bus = EventBus::new();
        let streaming_service = StreamingService::new(state.clone(), event_bus);

        tokio::spawn(async move {
            if let Err(e) =
                grpc::run_grpc_server_with_streaming(addr, state, streaming_service).await
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
