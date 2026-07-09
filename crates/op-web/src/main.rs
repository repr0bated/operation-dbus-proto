//! op-web: Main Entry Point
//!
//! Unified web server for op-dbus-v2.

use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use op_web::routes;
use op_web::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging with environment filter (RUST_LOG)
    // Default to info if not set
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .try_init();

    info!("Starting op-web server...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Absolute Base: a plugin exists ⟺ its sealed blob is in the catalog.
    // Require the blob-catalog manifest (not the removed live-schema.json monolith).
    const BLOB_MANIFEST: &str = "/dev/shm/opdbus/plugin-blobs/.manifest.json";
    match op_blob::catalog::read_manifest_plugin_ids_shm() {
        Some(ids) if !ids.is_empty() => {
            info!(
                path = BLOB_MANIFEST,
                plugins = ids.len(),
                "Sealed blob catalog present in shared memory"
            );
        }
        _ => {
            tracing::error!(
                path = BLOB_MANIFEST,
                "FATAL: Blob catalog missing. Run `opblob seal-shm` or start op-grpc-bridge first."
            );
            std::process::exit(1);
        }
    }

    let state = Arc::new(AppState::new().await?);
    state.clone().start_event_bridge();
    state.clone().start_system_monitor();
    let app = routes::create_router(state);

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on http://{}", addr);
    info!("WebSocket available at ws://{}/ws", addr);
    info!("API available at http://{}/api", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
