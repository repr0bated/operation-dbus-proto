//! Tonic gRPC server wiring. Registers every Assistant gateway service,
//! reflection, health checks, the WireGuard auth interceptor, and an optional
//! gRPC-Web layer for browser clients.

use crate::agents::AgentServiceImpl;
use crate::auth::wireguard_auth_interceptor;
use crate::client::AssistantClient;
use crate::cron::CronServiceImpl;
use crate::memory::MemoryServiceImpl;
use crate::models::ModelServiceImpl;
use crate::namespace::NamespaceMemoryServiceImpl;
use crate::proto::agent_service_server::AgentServiceServer;
use crate::proto::cron_service_server::CronServiceServer;
use crate::proto::memory_service_server::MemoryServiceServer;
use crate::proto::model_service_server::ModelServiceServer;
use crate::proto::namespace_memory_service_server::NamespaceMemoryServiceServer;
use crate::proto::session_service_server::SessionServiceServer;
use crate::proto::soul_service_server::SoulServiceServer;
use crate::proto::task_service_server::TaskServiceServer;
use crate::sessions::SessionServiceImpl;
use crate::soul::SoulServiceImpl;
use crate::tasks::TaskServiceImpl;
use crate::transport::TransportConfig;
use op_cognitive_mcp::cozo_shuttle::CozoGraphShuttle;
use op_cognitive_mcp::memory_store::CognitiveMemoryStore;
use op_cognitive_mcp::soul_memory::SoulMemoryStore;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;

pub const DEFAULT_GRPC_PORT: u16 = 50051;
pub const DEFAULT_GRPC_HOST: &str = "0.0.0.0";

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub transport: TransportConfig,
    pub enable_grpc_web: bool,
    pub enable_reflection: bool,
    /// CozoDB path backing memory / soul / namespace stores. Empty = in-memory.
    pub cozo_db_path: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: std::env::var("OP_ASSISTANT_GRPC_HOST")
                .unwrap_or_else(|_| DEFAULT_GRPC_HOST.to_string()),
            port: std::env::var("OP_ASSISTANT_GRPC_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(DEFAULT_GRPC_PORT),
            transport: TransportConfig::default(),
            enable_grpc_web: std::env::var("OP_ASSISTANT_GRPC_WEB")
                .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
                .unwrap_or(false),
            enable_reflection: true,
            cozo_db_path: std::env::var("OP_ASSISTANT_COZO_PATH").unwrap_or_default(),
        }
    }
}

pub struct AssistantGrpcServer {
    cfg: ServerConfig,
    client: AssistantClient,
    memory_store: Arc<CognitiveMemoryStore>,
    soul_store: Arc<SoulMemoryStore>,
}

impl AssistantGrpcServer {
    pub async fn new(cfg: ServerConfig) -> anyhow::Result<Self> {
        let client = AssistantClient::new(cfg.transport.clone()).await?;

        let shuttle = if cfg.cozo_db_path.is_empty() {
            CozoGraphShuttle::new_in_memory()?
        } else {
            CozoGraphShuttle::new_persistent(PathBuf::from(&cfg.cozo_db_path))?
        };
        let shuttle = Arc::new(shuttle);
        let memory_store = Arc::new(CognitiveMemoryStore::new(shuttle.clone()).await?);
        let soul_store = Arc::new(SoulMemoryStore::new(shuttle));

        Ok(Self {
            cfg,
            client,
            memory_store,
            soul_store,
        })
    }

    pub fn client(&self) -> &AssistantClient {
        &self.client
    }

    pub fn memory_store(&self) -> Arc<CognitiveMemoryStore> {
        self.memory_store.clone()
    }

    pub fn soul_store(&self) -> Arc<SoulMemoryStore> {
        self.soul_store.clone()
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.cfg.host, self.cfg.port).parse()?;
        info!(%addr, transport = ?self.client.transport().primary_kind(), "starting op-assistant-grpc");
        // Publish D-Bus interface so model calls work over the session bus.
        let _dbus_conn = crate::dbus_service::serve(Arc::new(self.client.clone())).await?;

        let agent = AgentServiceServer::with_interceptor(
            AgentServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let session = SessionServiceServer::with_interceptor(
            SessionServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let task = TaskServiceServer::with_interceptor(
            TaskServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let model = ModelServiceServer::with_interceptor(
            ModelServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let cron = CronServiceServer::with_interceptor(
            CronServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let soul = SoulServiceServer::with_interceptor(
            SoulServiceImpl::new(self.soul_store.clone()),
            wireguard_auth_interceptor,
        );
        let namespace = NamespaceMemoryServiceServer::with_interceptor(
            NamespaceMemoryServiceImpl::new(self.memory_store.clone(), self.soul_store.clone()),
            wireguard_auth_interceptor,
        );
        let memory = MemoryServiceServer::with_interceptor(
            MemoryServiceImpl::new(self.memory_store.clone()),
            wireguard_auth_interceptor,
        );

        let mut builder = Server::builder()
            .accept_http1(true)
            .add_service(tower::Layer::layer(&tonic_web::GrpcWebLayer::new(), agent))
            .add_service(tower::Layer::layer(
                &tonic_web::GrpcWebLayer::new(),
                session,
            ))
            .add_service(tower::Layer::layer(&tonic_web::GrpcWebLayer::new(), task))
            .add_service(tower::Layer::layer(&tonic_web::GrpcWebLayer::new(), model))
            .add_service(tower::Layer::layer(&tonic_web::GrpcWebLayer::new(), cron))
            .add_service(tower::Layer::layer(&tonic_web::GrpcWebLayer::new(), soul))
            .add_service(tower::Layer::layer(
                &tonic_web::GrpcWebLayer::new(),
                namespace,
            ))
            .add_service(tower::Layer::layer(&tonic_web::GrpcWebLayer::new(), memory));

        if self.cfg.enable_reflection {
            let reflection = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
                .build_v1()?;
            builder = builder.add_service(tower::Layer::layer(
                &tonic_web::GrpcWebLayer::new(),
                reflection,
            ));
        }

        let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
        health_reporter
            .set_serving::<AgentServiceServer<AgentServiceImpl>>()
            .await;
        builder = builder.add_service(tower::Layer::layer(
            &tonic_web::GrpcWebLayer::new(),
            health_service,
        ));

        builder.serve(addr).await?;
        Ok(())
    }
}

/// Convenience entry-point used by `op-dbus` and integration tests.
pub async fn run_grpc_server(cfg: ServerConfig) -> anyhow::Result<()> {
    AssistantGrpcServer::new(cfg).await?.serve().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_populate() {
        let cfg = ServerConfig::default();
        assert!(cfg.port > 0);
        assert!(!cfg.host.is_empty());
    }
}
