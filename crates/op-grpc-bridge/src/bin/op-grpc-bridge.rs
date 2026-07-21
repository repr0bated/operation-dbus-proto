//! Consolidated Operation gRPC bridge.
//!
//! The former `op-grpc-bridge-zeroclaw` entry point is folded into this main
//! service. It exposes the complete Operation route set plus the Zeroclaw
//! schema API on TCP port 8090 and the plugin Unix socket.

use op_grpc_bridge::server::{run_zeroclaw_server, ServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let config = ServerConfig::from_env();
    tracing::info!(
        plugin_id = %config.plugin_id,
        schema_path = %config.schema_path.display(),
        unix_socket = %config.unix_socket.display(),
        bind_addr = %config.bind_addr,
        tls_enabled = config.tls_bind_addr.is_some(),
        "Consolidated Operation gRPC bridge starting"
    );
    run_zeroclaw_server(config).await
}
