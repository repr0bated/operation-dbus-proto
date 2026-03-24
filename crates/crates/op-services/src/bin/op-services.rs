//! op-services daemon

use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::EnvFilter;

use op_services::dbus::interface::run_dbus_server;
use op_services::grpc::proto::service_manager_server::ServiceManagerServer;
use op_services::grpc::server::GrpcServer;
use op_services::manager::ServiceManager;
use op_services::store::Store;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("op_services=info".parse()?))
        .init();

    info!("Starting op-services daemon");

    // Initialize store
    let store = Arc::new(Store::new("/var/lib/op-dbus/services.db").await?);

    // Initialize service manager
    let manager = Arc::new(ServiceManager::new(store).await?);

    // Start D-Bus interface in background
    let dbus_manager = manager.clone();
    tokio::spawn(async move {
        if let Err(e) = run_dbus_server(dbus_manager).await {
            tracing::error!("D-Bus server error: {}", e);
        }
    });

    // Start gRPC server
    let grpc_server = GrpcServer::new(manager);
    let addr = "[::]:50051".parse()?;

    info!("gRPC server listening on {}", addr);

    Server::builder()
        .add_service(ServiceManagerServer::new(grpc_server))
        .serve(addr)
        .await?;

    Ok(())
}
