use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let socket = std::env::var("ADAPTERS_SOCKET")
        .unwrap_or_else(|_| "/run/op-grpc-adapters.sock".to_string());

    op_grpc_adapters::server::run(&socket).await
}
