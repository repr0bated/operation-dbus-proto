//! gRPC server — binds all adapters on a single unix socket with tonic-web,
//! reflection, health, and Ghostbridge interceptor.

use anyhow::Result;
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder as ReflectionBuilder;

use crate::adapters::{mail::MailAdapter, mq::MqAdapter, netmaker::NetmakerAdapter};
use crate::interceptor::ghostbridge_interceptor;
use crate::proto::{
    mail_service_server::MailServiceServer, mq_service_server::MqServiceServer,
    netmaker_service_server::NetmakerServiceServer, FILE_DESCRIPTOR_SET,
};

pub async fn run(socket_path: &str) -> Result<()> {
    // Remove stale socket file
    let _ = std::fs::remove_file(socket_path);

    let uds = tokio::net::UnixListener::bind(socket_path)?;
    let uds_stream = tokio_stream::wrappers::UnixListenerStream::new(uds);

    let (mut health_reporter, health_service) = health_reporter();
    health_reporter.set_serving::<MailServiceServer<MailAdapter>>().await;
    health_reporter.set_serving::<NetmakerServiceServer<NetmakerAdapter>>().await;
    health_reporter.set_serving::<MqServiceServer<MqAdapter>>().await;

    let reflection = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    tracing::info!(socket = socket_path, "op-grpc-adapters listening");

    Server::builder()
        .accept_http1(true)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(health_service)
        .add_service(reflection)
        .add_service(MailServiceServer::with_interceptor(MailAdapter::new(), ghostbridge_interceptor))
        .add_service(NetmakerServiceServer::with_interceptor(NetmakerAdapter::new(), ghostbridge_interceptor))
        .add_service(MqServiceServer::with_interceptor(MqAdapter::new(), ghostbridge_interceptor))
        .serve_with_incoming(uds_stream)
        .await?;

    Ok(())
}
