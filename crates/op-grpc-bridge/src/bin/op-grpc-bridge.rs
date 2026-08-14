//! Consolidated Operation gRPC bridge.
//!
//! The former `op-grpc-bridge-zeroclaw` entry point is folded into this main
//! service. It exposes the complete Operation route set plus the Zeroclaw
//! schema API on TCP port 8090 and the plugin Unix socket.

use op_grpc_bridge::server::{run_zeroclaw_server, ServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tonic 0.12 and Qdrant/Reqwest enable different rustls crypto backends.
    // Select the provider used by this crate's TLS integration tests before
    // constructing any ServerTlsConfig, otherwise rustls refuses to guess.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("rustls CryptoProvider was already installed"))?;

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
        shared_socket = %config.shared_socket.display(),
        bind_addr = %config.bind_addr,
        tls_enabled = config.tls_identity.is_some(),
        "Consolidated Operation gRPC bridge starting"
    );
    run_zeroclaw_server(config).await
}
