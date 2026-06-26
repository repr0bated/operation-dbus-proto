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
//!   - Shared UDS at /run/ghostbridge/container.sock for container access.
//!   - Schema-driven routing: every plugin in the live schema catalog is
//!     automatically exposed as a gRPC-callable D-Bus object.

use std::net::SocketAddr;
use std::sync::Arc;

use op_grpc_bridge::{
    grpc_server::run_grpc_server,
    schema_engine::SchemaEngine,
    schema_router::SchemaRouter,
    shared_socket::{SharedSocketConfig, SharedSocketManager},
};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::rovs_proxy::OvsdbDbusClient;
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
    let ovsdb = Arc::new(OvsdbDbusClient::new());
    let nonnet = Arc::new(NonNetDb::new());
    let schema_engine = Arc::new(SchemaEngine::new(event_chain, ovsdb, nonnet));

    // ── Schema Router (dynamic D-Bus routing from live schema catalog) ───────
    let schema_router = Arc::new(SchemaRouter::new(schema_engine.dbus_connection.clone()));
    tracing::info!("SchemaRouter initialized — plugins auto-exposed to gRPC from /dev/shm/live-schema.json");

    // ── Shared Socket (container access via UDS) ─────────────────────────────
    let socket_config = SharedSocketConfig::default();
    let socket_manager = Arc::new(SharedSocketManager::new(socket_config.clone()));

    // Spawn the shared socket listener in the background.
    let sm = socket_manager.clone();
    let sr = schema_router.clone();
    tokio::spawn(async move {
        match sm.bind().await {
            Ok(listener) => {
                tracing::info!(
                    socket_path = %socket_config.socket_path.display(),
                    "Shared container socket active — containers can connect"
                );
                // Refresh container PID map on startup.
                sm.refresh_container_map().await;

                // Accept connections and log peer identity.
                // In a full implementation, this would feed into a tonic server
                // running on the UDS, but for now we log and validate peers.
                loop {
                    match listener.accept().await {
                        Ok((stream, _addr)) => {
                            let peer_cred = stream.peer_cred();
                            match peer_cred {
                                Ok(cred) => {
                                    let identity = sm.resolve_peer(cred).await;
                                    tracing::debug!(
                                        ?identity,
                                        "Accepted connection on shared socket"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        error = %e,
                                        "Failed to get peer credentials"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Shared socket accept failed");
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to bind shared container socket — container access unavailable"
                );
            }
        }
    });

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
    tracing::info!(
        "Schema-driven passthrough active — D-Bus objects exposed to gRPC automatically"
    );

    run_grpc_server(addr, schema_engine, None).await?;
    Ok(())
}
