//! The Shuttle — gRPC Bridge Binary
//!
//! Zero-trust gRPC gateway enforcing the Absolute Base rule via
//! GhostbridgeInterceptor. Reads the IdentitySled from shared memory
//! (/dev/shm/plugin_schema.dat) and rejects any request whose footprint
//! does not match the current Strike/Etch.
//!
//! Dual listener:
//!   - TCP on GRPC_BIND (default 127.0.0.1:18789) — Xray redirect target.
//!   - UDS on GHOSTBRIDGE_SOCKET_PATH (default /run/ghostbridge/container.sock)
//!     — container access with full tonic stack and identity injection.
//!
//! SchemaRouter is wired into the live DbusPassthrough path: every plugin in
//! the SHM catalog is automatically routable without hand-rolled services.

use std::net::SocketAddr;
use std::sync::Arc;

use op_grpc_bridge::{
    grpc_server::{run_grpc_server, OperationGrpcServer},
    schema_engine::SchemaEngine,
    schema_router::SchemaRouter,
    shared_socket,
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

    // Authoritative D-Bus object registration:
    // The bridge brings the schema-defined objects into existence on the bus.
    let sr_init = schema_router.clone();
    let se_init = schema_engine.clone();
    tokio::spawn(async move {
        // Wait for D-Bus connection to be ready
        if let Ok(conn) = se_init.dbus_connection().await {
            if let Err(e) = sr_init.register_objects().await {
                tracing::error!(error = %e, "Failed to register authoritative D-Bus objects");
            } else {
                // Request the canonical bus name for the bridge
                if let Err(e) = conn.request_name(op_plugins::canonical::BASE_SERVICE_NAME).await {
                    tracing::warn!(error = %e, "Failed to request D-Bus name (likely already owned)");
                }
            }
        }
    });

    let plugin_count = schema_router.list_plugin_ids().await.len();
    tracing::info!(
        plugin_count,
        "SchemaRouter initialized — all plugins auto-exposed to gRPC"
    );

    // ── Shared UDS (container access — full tonic stack) ────────────────────
    let se_uds = schema_engine.clone();
    let sr_uds = schema_router.clone();
    tokio::spawn(async move {
        match shared_socket::bind_shared_socket().await {
            Ok(incoming) => {
                tracing::info!("Shared container socket active — serving full gRPC stack over UDS");

                use op_grpc_bridge::proto::dbus_passthrough_server::DbusPassthroughServer;
                use op_grpc_bridge::proto::plugin_service_server::PluginServiceServer;
                use op_grpc_bridge::proto::state_sync_server::StateSyncServer;

                let server = OperationGrpcServer::new(se_uds)
                    .with_schema_router(sr_uds);

                let result = tonic::transport::Server::builder()
                    .add_service(DbusPassthroughServer::with_interceptor(
                        server.clone(),
                        shared_socket::uds_identity_interceptor,
                    ))
                    .add_service(PluginServiceServer::with_interceptor(
                        server.clone(),
                        shared_socket::uds_identity_interceptor,
                    ))
                    .add_service(StateSyncServer::with_interceptor(
                        server.clone(),
                        shared_socket::uds_identity_interceptor,
                    ))
                    .serve_with_incoming(incoming)
                    .await;

                if let Err(e) = result {
                    tracing::error!(error = %e, "UDS gRPC server exited with error");
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

    // ── TCP Bind address ────────────────────────────────────────────────────
    let addr: SocketAddr = std::env::var("GRPC_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18789".to_string())
        .parse()
        .expect("GRPC_BIND must be a valid socket address");

    tracing::info!(%addr, "The Shuttle gRPC bridge starting (TCP)");
    tracing::info!("GhostbridgeInterceptor active — all requests identity-gated");
    tracing::info!(
        "SchemaRouter: {} plugins auto-exposed via DbusPassthrough",
        plugin_count
    );

    run_grpc_server(addr, schema_engine, None).await?;
    Ok(())
}
