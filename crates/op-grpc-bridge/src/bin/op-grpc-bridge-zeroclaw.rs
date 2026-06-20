//! 🟢 🦅 ZeroClaw Axum Host — HTTP/gRPC-Web schema server.
//!
//! Reads the plugin-owned zeroclaw schema from `/dev/shm/opdbus/schemas/zeroclaw.json`
//! and serves it over native gRPC (Unix socket) and gRPC-Web/HTTP (TCP).
//! This is the HTTP-facing side of The Shuttle for the zeroclaw plugin.

use op_grpc_bridge::server::{run_zeroclaw_server, ServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("op_grpc_bridge=info".parse()?)
                .add_directive("info".parse()?),
        )
        .init();

    let config = ServerConfig::from_env();
    tracing::info!(?config, "ZeroClaw Axum host starting");
    run_zeroclaw_server(config).await
}
