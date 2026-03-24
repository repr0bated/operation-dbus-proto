//! gRPC server setup and configuration

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use tonic::transport::Server;
use tracing::info;

use super::agent_service::AgentServiceImpl;
use super::cache_service::CacheServiceImpl;
use super::orchestrator_service::OrchestratorServiceImpl;
use super::proto::{
    agent_service_server::AgentServiceServer, cache_service_server::CacheServiceServer,
    orchestrator_service_server::OrchestratorServiceServer,
};

/// Server configuration
#[derive(Debug, Clone)]
pub struct GrpcServerConfig {
    pub listen_addr: SocketAddr,
    pub workstack_threshold: usize,
    pub enable_caching: bool,
    pub promotion_threshold: u32,
    pub default_cache_ttl_secs: i64,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "[::1]:50051".parse().unwrap(),
            workstack_threshold: 2,
            enable_caching: true,
            promotion_threshold: 3,
            default_cache_ttl_secs: 3600,
        }
    }
}

/// gRPC server builder
pub struct GrpcServer {
    config: GrpcServerConfig,
    agent_service: Arc<AgentServiceImpl>,
    cache_service: Arc<CacheServiceImpl>,
    orchestrator_service: Arc<OrchestratorServiceImpl>,
}

impl GrpcServer {
    /// Create new gRPC server with default configuration
    pub fn new() -> Self {
        Self::with_config(GrpcServerConfig::default())
    }

    /// Create new gRPC server with custom configuration
    pub fn with_config(config: GrpcServerConfig) -> Self {
        let agent_service = Arc::new(AgentServiceImpl::new());
        let cache_service = Arc::new(CacheServiceImpl::with_ttl(config.default_cache_ttl_secs));
        let orchestrator_service = Arc::new(OrchestratorServiceImpl::with_config(
            agent_service.clone(),
            cache_service.clone(),
            config.workstack_threshold,
            config.enable_caching,
            config.promotion_threshold,
        ));

        Self {
            config,
            agent_service,
            cache_service,
            orchestrator_service,
        }
    }

    /// Get agent service for local registration
    pub fn agent_service(&self) -> Arc<AgentServiceImpl> {
        self.agent_service.clone()
    }

    /// Get orchestrator service
    pub fn orchestrator_service(&self) -> Arc<OrchestratorServiceImpl> {
        self.orchestrator_service.clone()
    }

    /// Get cache service
    pub fn cache_service(&self) -> Arc<CacheServiceImpl> {
        self.cache_service.clone()
    }

    /// Start the gRPC server
    pub async fn serve(self) -> Result<()> {
        let addr = self.config.listen_addr;

        info!("Starting gRPC server on {}", addr);

        Server::builder()
            .add_service(AgentServiceServer::from_arc(self.agent_service))
            .add_service(CacheServiceServer::from_arc(self.cache_service))
            .add_service(OrchestratorServiceServer::from_arc(
                self.orchestrator_service,
            ))
            .serve(addr)
            .await?;

        Ok(())
    }

    /// Serve with graceful shutdown
    pub async fn serve_with_shutdown(
        self,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> Result<()> {
        let addr = self.config.listen_addr;

        info!("Starting gRPC server on {} (with graceful shutdown)", addr);

        Server::builder()
            .add_service(AgentServiceServer::from_arc(self.agent_service))
            .add_service(CacheServiceServer::from_arc(self.cache_service))
            .add_service(OrchestratorServiceServer::from_arc(
                self.orchestrator_service,
            ))
            .serve_with_shutdown(addr, shutdown)
            .await?;

        Ok(())
    }
}

impl Default for GrpcServer {
    fn default() -> Self {
        Self::new()
    }
}
