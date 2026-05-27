//! op-assistant-grpc — gRPC gateway for the self-hosted Assistant.
//!
//! Routes gRPC calls through the wg-xray Incus container's `op-grpc-bridge`
//! endpoint (default `10.200.0.1:50051`) with D-Bus-first transport and
//! ghostbridge schema-tag header injection for Xray OpenFlow routing.

use anyhow::Result;
use op_assistant_grpc::{run_grpc_server, ServerConfig};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("op_assistant_grpc=info,info")),
        )
        .init();

    let cfg = ServerConfig::default();
    info!(
        host = %cfg.host,
        port = cfg.port,
        endpoint = %cfg.transport.rpc_endpoint,
        "op-assistant-grpc starting"
    );

    run_grpc_server(cfg).await
}
