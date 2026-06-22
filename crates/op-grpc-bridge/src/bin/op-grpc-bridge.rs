//! 🟢 🛷 The Shuttle — gRPC Bridge Binary
//!
//! Zero-trust gRPC gateway enforcing the Absolute Base rule via
//! GhostbridgeInterceptor. Reads the IdentitySled from shared memory
//! (/dev/shm/plugin_schema.dat) and rejects any request whose footprint
//! does not match the current Strike/Etch.
//!
//! Design:
//!   - Does NOT write the sled; the MutationEngine or A.N.N.A. Scribe does.
//!   - If no valid sled exists, all inbound requests are rejected.
//!   - Bind address defaults to 127.0.0.1:18789 (Xray redirect target).

use std::net::SocketAddr;
use std::sync::Arc;

use op_grpc_bridge::{grpc_server::run_grpc_server, mutation_engine::MutationEngine};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::rovs_proxy::OvsdbDbusClient;
use op_state_store::{ChainConfig, EventChain};
use tokio::sync::RwLock;
use tonic::transport::Identity;

/// Load TLS identity from env vars or auto-generate a self-signed cert.
/// Env: GRPC_TLS_CERT (PEM), GRPC_TLS_KEY (PEM). If both absent, generates one.
fn load_tls_identity() -> Option<Identity> {
    let cert_pem = std::env::var("GRPC_TLS_CERT").ok();
    let key_pem = std::env::var("GRPC_TLS_KEY").ok();

    match (cert_pem, key_pem) {
        (Some(cert), Some(key)) => {
            tracing::info!("TLS identity loaded from GRPC_TLS_CERT/GRPC_TLS_KEY env vars");
            Some(Identity::from_pem(cert, key))
        }
        _ => {
            let ck = rcgen::generate_simple_self_signed(vec![
                "localhost".to_string(),
                "ghostbridge.tech".to_string(),
                "3tched.com".to_string(),
            ])
            .expect("failed to generate self-signed TLS cert");
            let cert_pem = ck.cert.pem();
            let key_pem = ck.key_pair.serialize_pem();
            tracing::info!("TLS identity auto-generated (self-signed for localhost, ghostbridge.tech, 3tched.com)");
            Some(Identity::from_pem(cert_pem, key_pem))
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("op_grpc_bridge=info".parse()?)
                .add_directive("info".parse()?),
        )
        .init();

    // ── Build MutationEngine (authoritative mutation pipeline) ─────────────────
    let event_chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
    let ovsdb = Arc::new(OvsdbDbusClient::new());
    let nonnet = Arc::new(NonNetDb::new());
    let mutation_engine = Arc::new(MutationEngine::new(event_chain, ovsdb, nonnet));

    // ── Bind address ─────────────────────────────────────────────────────────
    // Per spec: Xray redirects gRPC traffic to 127.0.0.1:18789.
    let addr: SocketAddr = std::env::var("GRPC_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18789".to_string())
        .parse()
        .expect("GRPC_BIND must be a valid socket address");

    // ── TLS identity ────────────────────────────────────────────────────────
    let tls_identity = load_tls_identity();

    tracing::info!(%addr, "The Shuttle gRPC bridge starting");
    tracing::info!(
        "GhostbridgeInterceptor active — requests require X-Ghostbridge-Footprint + X-Ghostbridge-Trace-ID"
    );

    run_grpc_server(addr, mutation_engine, None, tls_identity).await?;
    Ok(())
}
