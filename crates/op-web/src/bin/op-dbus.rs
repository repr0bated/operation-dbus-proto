//! op-dbus: gRPC server for all Operation services
//!
//! Binds to 10.88.88.1:50051 (ovsbr0) so all Incus containers can reach it
//! through the single OVS choke point.
//!
//! Listen address override: OP_DBUS_GRPC_LISTEN (e.g. "0.0.0.0:50051" for dev)

use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use op_grpc_bridge::run_grpc_server;
use op_grpc_bridge::SyncEngine;
use op_state_store::{ChainConfig, EventChain};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init();

    let listen =
        std::env::var("OP_DBUS_GRPC_LISTEN").unwrap_or_else(|_| "10.88.88.1:50051".to_string());
    let addr: std::net::SocketAddr = listen
        .parse()
        .unwrap_or_else(|_| "10.88.88.1:50051".parse().unwrap());

    let chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
    let engine = Arc::new(SyncEngine::new(chain));

    info!(addr = %addr, "op-dbus starting");
    run_grpc_server(addr, engine, None).await?;

    Ok(())
}
