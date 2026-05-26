//! OpenFlow 1.3 controller for ovsbr0
//!
//! Listens on OF_CONTROLLER_LISTEN (default 10.200.0.1:6653) for OVS to
//! connect, then installs bidirectional flows between the configured port pairs.
//!
//! Environment variables:
//!   OF_CONTROLLER_LISTEN   listen address (default: 10.200.0.1:6653)
//!   OF_FLOW_PAIRS          comma-separated port pairs, e.g. "grpc-bridge:ovsbr0-sock"
//!                          defaults to "grpc-bridge:ovsbr0-sock"
//!   OF_FLOW_PRIORITY       flow priority (default: 100)

use std::net::SocketAddr;

use anyhow::Result;
use op_network::OpenFlowController;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("op_network=info".parse()?))
        .init();

    let listen: SocketAddr = std::env::var("OF_CONTROLLER_LISTEN")
        .unwrap_or_else(|_| "10.200.0.1:6653".to_string())
        .parse()
        .expect("OF_CONTROLLER_LISTEN must be a valid socket address");

    let pairs_env =
        std::env::var("OF_FLOW_PAIRS").unwrap_or_else(|_| "grpc-bridge:ovsbr0-sock".to_string());

    let priority: u16 = std::env::var("OF_FLOW_PRIORITY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let mut controller = OpenFlowController::new(listen);

    for pair in pairs_env.split(',') {
        let parts: Vec<&str> = pair.trim().splitn(2, ':').collect();
        if parts.len() != 2 {
            tracing::warn!("Ignoring malformed flow pair: {:?}", pair);
            continue;
        }
        info!(
            "Flow pair: {} ↔ {} (priority {})",
            parts[0], parts[1], priority
        );
        controller = controller.add_port_pair(parts[0], parts[1], priority);
    }

    controller.run().await
}
