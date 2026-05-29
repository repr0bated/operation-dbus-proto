//! 🟢 🛷 The Shuttle — gRPC Bridge Binary
//!
//! Zero-trust gRPC gateway enforcing the Absolute Base rule via
//! GhostbridgeInterceptor. Reads the IdentitySled from shared memory
//! (/dev/shm/plugin_schema.dat) and rejects any request whose footprint
//! does not match the current Strike/Etch.
//!
//! Design:
//!   - Does NOT write the sled; the SchemaEngine or A.N.N.A. Scribe does.
//!   - If no valid sled exists, all inbound requests are rejected.
//!   - Bind address defaults to 127.0.0.1:18789 (Xray redirect target).

use std::net::SocketAddr;
use std::sync::Arc;

use op_grpc_bridge::{grpc_server::run_grpc_server, schema_engine::SchemaEngine};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use op_state_store::{ChainConfig, EventChain};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("op_grpc_bridge=info".parse()?)
                .add_directive("info".parse()?),
        )
        .init();

    // ── Build SchemaEngine (authoritative mutation pipeline) ─────────────────
    let event_chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
    let ovsdb = Arc::new(OvsdbClient::new());
    let nonnet = Arc::new(NonNetDb::new());
    let schema_engine = Arc::new(SchemaEngine::new(event_chain, ovsdb, nonnet));

    // ── Bind address ─────────────────────────────────────────────────────────
    // Per spec: Xray redirects gRPC traffic to 127.0.0.1:18789.
    let addr: SocketAddr = std::env::var("GRPC_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18789".to_string())
        .parse()
        .expect("GRPC_BIND must be a valid socket address");

    tracing::info!(%addr, "The Shuttle gRPC bridge starting");
    tracing::info!(
        "GhostbridgeInterceptor active — requests require X-Ghostbridge-Footprint + X-Ghostbridge-Trace-ID"
    );

    run_grpc_server(addr, schema_engine, None).await?;
    Ok(())
}
