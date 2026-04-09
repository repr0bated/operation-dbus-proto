//! D-Bus Agent Manager
//!
//! Starts and manages all agents as D-Bus services.
//! Run this as a systemd service to have agents available.
//!
//! Each agent registers on D-Bus as:
//!   - Service: org.dbusmcp.Agent.{AgentType}
//!   - Path: /org/dbusmcp/Agent/{AgentType}
//!   - Interface: org.dbusmcp.Agent
//!
//! The ChatActor's tool_loader discovers these via introspection.

use anyhow::Result;
use op_agents::{
    agents::base::{AgentTask, AgentTrait},
    create_agent,
    dbus_service::{start_agent, DbusAgentService},
};
use op_core::BusType;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{error, info};
use zbus::Connection;

// Include generated proto types from op-chat
#[allow(warnings)]
pub mod proto {
    include!("../../../op-chat/src/orchestration/proto/op_chat.orchestration.rs");
}
use proto::{
    agent_execution_server::{AgentExecution, AgentExecutionServer},
    agent_lifecycle_server::{AgentLifecycle, AgentLifecycleServer},
    AgentInfo, AgentStatusEvent, BatchExecuteRequest, CancelRequest, CancelResponse,
    EndSessionRequest, EndSessionResponse, ExecuteChunk, ExecuteError, ExecuteRequest,
    ExecuteResponse, HealthCheckRequest, HealthCheckResponse,
    ShutdownRequest as ProtoShutdownRequest, ShutdownResponse as ProtoShutdownResponse,
    StartSessionRequest, StartSessionResponse, WatchAgentsRequest,
};

/// Agent configuration
struct AgentConfig {
    agent_type: &'static str,
    auto_start: bool,
    priority: u8,
}

/// Agents to start (run-on-connection + on-demand)
const AGENTS: &[AgentConfig] = &[
    AgentConfig {
        agent_type: "memory",
        auto_start: true,
        priority: 100,
    },
    AgentConfig {
        agent_type: "context-manager",
        auto_start: true,
        priority: 100,
    },
    AgentConfig {
        agent_type: "sequential-thinking",
        auto_start: true,
        priority: 100,
    },
    AgentConfig {
        agent_type: "dx-optimizer",
        auto_start: true,
        priority: 95,
    },
    AgentConfig {
        agent_type: "tdd-orchestrator",
        auto_start: true,
        priority: 95,
    },
    AgentConfig {
        agent_type: "rust-pro",
        auto_start: true,
        priority: 90,
    },
    AgentConfig {
        agent_type: "python-pro",
        auto_start: true,
        priority: 90,
    },
    AgentConfig {
        agent_type: "backend-architect",
        auto_start: true,
        priority: 90,
    },
    AgentConfig {
        agent_type: "frontend-developer",
        auto_start: true,
        priority: 90,
    },
    AgentConfig {
        agent_type: "database-architect",
        auto_start: true,
        priority: 85,
    },
    AgentConfig {
        agent_type: "backend-security-coder",
        auto_start: true,
        priority: 85,
    },
    AgentConfig {
        agent_type: "network-engineer",
        auto_start: true,
        priority: 80,
    },
    AgentConfig {
        agent_type: "deployment",
        auto_start: true,
        priority: 80,
    },
    AgentConfig {
        agent_type: "devops-troubleshooter",
        auto_start: true,
        priority: 80,
    },
    AgentConfig {
        agent_type: "debugger",
        auto_start: true,
        priority: 75,
    },
    AgentConfig {
        agent_type: "code-reviewer",
        auto_start: true,
        priority: 75,
    },
    AgentConfig {
        agent_type: "search-specialist",
        auto_start: true,
        priority: 75,
    },
    AgentConfig {
        agent_type: "prompt-engineer",
        auto_start: true,
        priority: 70,
    },
    AgentConfig {
        agent_type: "docs-architect",
        auto_start: true,
        priority: 70,
    },
];

#[derive(Clone)]
struct GrpcAgentService {
    sessions: Arc<RwLock<HashMap<String, HashMap<String, Arc<dyn AgentTrait + Send + Sync>>>>>,
}

#[tonic::async_trait]
impl AgentLifecycle for GrpcAgentService {
    async fn start_session(
        &self,
        request: Request<StartSessionRequest>,
    ) -> Result<Response<StartSessionResponse>, Status> {
        let req = request.into_inner();
        let mut session_agents = HashMap::new();
        let mut started_agents = Vec::new();

        for agent_id in &req.requested_agents {
            match create_agent(agent_id, format!("{}-grpc", agent_id)) {
                Ok(agent) => {
                    let type_name = agent.name().to_string();
                    let agent_arc: Arc<dyn AgentTrait + Send + Sync> = agent.into();
                    session_agents.insert(agent_id.clone(), agent_arc);
                    started_agents.push(AgentInfo {
                        agent_id: agent_id.clone(),
                        agent_type: type_name,
                        status: 2, // Running
                        priority: 1,
                        operations: vec![],
                        started_at_ms: chrono::Utc::now().timestamp_millis(),
                    });
                }
                Err(_) => {
                    return Err(Status::not_found(format!("Agent not found: {}", agent_id)));
                }
            }
        }

        self.sessions
            .write()
            .await
            .insert(req.session_id.clone(), session_agents);

        Ok(Response::new(StartSessionResponse {
            success: true,
            session_id: req.session_id,
            started_agents,
            failed_agents: vec![],
            started_at_ms: chrono::Utc::now().timestamp_millis(),
        }))
    }

    async fn end_session(
        &self,
        request: Request<EndSessionRequest>,
    ) -> Result<Response<EndSessionResponse>, Status> {
        let req = request.into_inner();
        self.sessions.write().await.remove(&req.session_id);
        Ok(Response::new(EndSessionResponse {
            success: true,
            stopped_agents: vec![],
            session_duration_ms: 0,
        }))
    }

    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Err(Status::unimplemented("health_check"))
    }

    type WatchAgentsStream =
        tokio_stream::wrappers::ReceiverStream<Result<AgentStatusEvent, Status>>;
    async fn watch_agents(
        &self,
        _request: Request<WatchAgentsRequest>,
    ) -> Result<Response<Self::WatchAgentsStream>, Status> {
        Err(Status::unimplemented("watch_agents"))
    }

    async fn shutdown(
        &self,
        _request: Request<ProtoShutdownRequest>,
    ) -> Result<Response<ProtoShutdownResponse>, Status> {
        Err(Status::unimplemented("shutdown"))
    }
}

#[tonic::async_trait]
impl AgentExecution for GrpcAgentService {
    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> Result<Response<ExecuteResponse>, Status> {
        let req = request.into_inner();

        // Lazy-load agent if it wasn't requested during StartSession
        let agent = {
            let sessions = self.sessions.read().await;
            if let Some(agents) = sessions.get(&req.session_id) {
                if let Some(agent) = agents.get(&req.agent_id) {
                    Some(agent.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        let agent = match agent {
            Some(a) => a,
            None => {
                let created = create_agent(&req.agent_id, format!("{}-grpc-adhoc", req.agent_id))
                    .map_err(|_| {
                    Status::not_found(format!("Agent not found: {}", req.agent_id))
                })?;
                let arc: Arc<dyn AgentTrait + Send + Sync> = created.into();
                arc
            }
        };

        let mut config = HashMap::new();
        config.insert(
            "session_id".to_string(),
            simd_json::OwnedValue::String(req.session_id.clone().into()),
        );

        let task = AgentTask {
            task_type: req.agent_id.clone(),
            operation: req.operation.clone(),
            path: None,
            args: Some(req.arguments_json.clone()),
            config,
        };

        match agent.execute(task).await {
            Ok(result) => {
                let (result_json, error) = if result.success {
                    (result.data, None)
                } else {
                    (
                        "{}".to_string(),
                        Some(ExecuteError {
                            code: "EXEC_FAILED".to_string(),
                            message: result.data,
                            details: "".to_string(),
                            retryable: false,
                            stack_trace: "".to_string(),
                        }),
                    )
                };

                Ok(Response::new(ExecuteResponse {
                    correlation_id: req.correlation_id,
                    agent_id: req.agent_id,
                    operation: req.operation,
                    success: result.success,
                    result_json,
                    error,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                }))
            }
            Err(e) => Ok(Response::new(ExecuteResponse {
                correlation_id: req.correlation_id,
                agent_id: req.agent_id,
                operation: req.operation,
                success: false,
                result_json: "{}".to_string(),
                error: Some(ExecuteError {
                    code: "INTERNAL".to_string(),
                    message: e,
                    details: "".to_string(),
                    retryable: false,
                    stack_trace: "".to_string(),
                }),
                execution_time_ms: 0,
                metadata: HashMap::new(),
            })),
        }
    }

    type ExecuteStreamStream = tokio_stream::wrappers::ReceiverStream<Result<ExecuteChunk, Status>>;
    async fn execute_stream(
        &self,
        request: Request<ExecuteRequest>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let req = request.into_inner();

        let agent = {
            let sessions = self.sessions.read().await;
            if let Some(agents) = sessions.get(&req.session_id) {
                if let Some(agent) = agents.get(&req.agent_id) {
                    Some(agent.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        let agent = match agent {
            Some(a) => a,
            None => {
                let created = create_agent(&req.agent_id, format!("{}-grpc-adhoc", req.agent_id))
                    .map_err(|_| {
                    Status::not_found(format!("Agent not found: {}", req.agent_id))
                })?;
                let arc: Arc<dyn AgentTrait + Send + Sync> = created.into();
                arc
            }
        };

        let mut config = HashMap::new();
        config.insert(
            "session_id".to_string(),
            simd_json::OwnedValue::String(req.session_id.clone().into()),
        );

        let task = AgentTask {
            task_type: req.agent_id.clone(),
            operation: req.operation.clone(),
            path: None,
            args: Some(req.arguments_json.clone()),
            config,
        };

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            match agent.execute(task).await {
                Ok(result) => {
                    let (content, error) = if result.success {
                        (result.data, None)
                    } else {
                        (
                            "{}".to_string(),
                            Some(ExecuteError {
                                code: "EXEC_FAILED".to_string(),
                                message: result.data,
                                details: "".to_string(),
                                retryable: false,
                                stack_trace: "".to_string(),
                            }),
                        )
                    };

                    let _ = tx
                        .send(Ok(ExecuteChunk {
                            correlation_id: req.correlation_id.clone(),
                            chunk_type: 4, // Result
                            content,
                            is_final: true,
                            sequence: 0,
                            timestamp_ms: chrono::Utc::now().timestamp_millis(),
                            error,
                        }))
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(Ok(ExecuteChunk {
                            correlation_id: req.correlation_id.clone(),
                            chunk_type: 4, // Result
                            content: "{}".to_string(),
                            is_final: true,
                            sequence: 0,
                            timestamp_ms: chrono::Utc::now().timestamp_millis(),
                            error: Some(ExecuteError {
                                code: "INTERNAL".to_string(),
                                message: e,
                                details: "".to_string(),
                                retryable: false,
                                stack_trace: "".to_string(),
                            }),
                        }))
                        .await;
                }
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    type BatchExecuteStream =
        tokio_stream::wrappers::ReceiverStream<Result<ExecuteResponse, Status>>;
    async fn batch_execute(
        &self,
        _request: Request<BatchExecuteRequest>,
    ) -> Result<Response<Self::BatchExecuteStream>, Status> {
        Err(Status::unimplemented("batch_execute"))
    }

    async fn cancel(
        &self,
        _request: Request<CancelRequest>,
    ) -> Result<Response<CancelResponse>, Status> {
        Err(Status::unimplemented("cancel"))
    }
}

/// Agent Manager - starts and monitors D-Bus agent services
struct AgentManager {
    connections: HashMap<String, Connection>,
    bus_type: BusType,
}

impl AgentManager {
    fn new(bus_type: BusType) -> Self {
        Self {
            connections: HashMap::new(),
            bus_type,
        }
    }

    /// Start an agent as a D-Bus service
    async fn start_agent(&mut self, agent_type: &str) -> Result<()> {
        if self.connections.contains_key(agent_type) {
            info!("Agent {} already running", agent_type);
            return Ok(());
        }

        // Create the agent
        let agent_id = format!("{}-main", agent_type);
        let agent = create_agent(agent_type, agent_id.clone())
            .map_err(|e| anyhow::anyhow!("Failed to create agent {}: {}", agent_type, e))?;

        // Start as D-Bus service
        let connection = start_agent(agent, &agent_id, self.bus_type)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to start D-Bus service for {}: {}", agent_type, e)
            })?;

        let service_name = DbusAgentService::service_name(agent_type);
        info!("✓ Started D-Bus agent: {} at {}", agent_type, service_name);

        self.connections.insert(agent_type.to_string(), connection);
        Ok(())
    }

    /// Start all auto-start agents
    async fn start_auto_agents(&mut self) -> Result<()> {
        let mut started = 0;
        let mut failed = 0;

        // Sort by priority (highest first)
        let mut agents: Vec<_> = AGENTS.iter().filter(|a| a.auto_start).collect();
        agents.sort_by(|a, b| b.priority.cmp(&a.priority));

        for config in agents {
            match self.start_agent(config.agent_type).await {
                Ok(_) => started += 1,
                Err(e) => {
                    error!("Failed to start {}: {}", config.agent_type, e);
                    failed += 1;
                }
            }
        }

        info!(
            "Agent startup complete: {} started, {} failed",
            started, failed
        );
        Ok(())
    }

    /// List running agents
    fn list_running(&self) -> Vec<&str> {
        self.connections.keys().map(|s| s.as_str()).collect()
    }

    /// Stop an agent
    async fn stop_agent(&mut self, agent_type: &str) -> Result<()> {
        if let Some(_conn) = self.connections.remove(agent_type) {
            info!("Stopped agent: {}", agent_type);
            // Connection drops, D-Bus service unregisters
        }
        Ok(())
    }

    /// Stop all agents
    async fn stop_all(&mut self) {
        let agents: Vec<_> = self.connections.keys().cloned().collect();
        for agent in agents {
            let _ = self.stop_agent(&agent).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("op_agents=info".parse().unwrap()),
        )
        .init();

    info!("Starting op-dbus Agent Manager");

    // Determine bus type from environment
    let bus_type = if std::env::var("DBUS_AGENT_SESSION").is_ok() {
        info!("Using session bus");
        BusType::Session
    } else {
        info!("Using system bus");
        BusType::System
    };

    // Start gRPC server
    let grpc_port = std::env::var("OP_AGENT_GRPC_PORT").unwrap_or_else(|_| "50055".to_string());
    let addr = format!("127.0.0.1:{}", grpc_port).parse().unwrap();
    let grpc_service = GrpcAgentService {
        sessions: Arc::new(RwLock::new(HashMap::new())),
    };

    info!("Starting gRPC AgentService on {}", addr);
    let grpc_future = Server::builder()
        .add_service(AgentLifecycleServer::new(grpc_service.clone()))
        .add_service(AgentExecutionServer::new(grpc_service))
        .serve(addr);

    tokio::spawn(async move {
        if let Err(e) = grpc_future.await {
            error!("gRPC server error: {}", e);
        }
    });

    // Create manager and start agents
    let mut manager = AgentManager::new(bus_type);

    if let Err(e) = manager.start_auto_agents().await {
        error!("Failed to start D-Bus agents: {}", e);
        return Err(e);
    }

    info!(
        "Agent Manager ready. Running agents: {:?}",
        manager.list_running()
    );
    info!("Press Ctrl+C to stop");

    // Wait for shutdown signal
    signal::ctrl_c().await?;

    info!("Shutting down Agent Manager...");
    manager.stop_all().await;

    info!("Agent Manager stopped");
    Ok(())
}
