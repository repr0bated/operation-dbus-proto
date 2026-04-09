import re

with open('crates/op-agents/src/bin/dbus-agent-manager.rs', 'r') as f:
    content = f.read()

# 1. Add tonic and proto imports
imports = """use anyhow::Result;
use op_agents::{
    create_agent,
    agent_registry::AgentRegistry,
    agents::base::{AgentContext, AgentTask},
    dbus_service::{start_agent, DbusAgentService},
};
use op_core::BusType;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::signal;
use tracing::{error, info, warn};
use zbus::Connection;
use tonic::{transport::Server, Request, Response, Status};

// Include generated proto types from op-chat
#[allow(warnings)]
pub mod proto {
    include!("../../../crates/op-chat/src/orchestration/proto/op_chat.orchestration.rs");
}
use proto::{
    agent_execution_server::{AgentExecution, AgentExecutionServer},
    agent_lifecycle_server::{AgentLifecycle, AgentLifecycleServer},
    ExecuteRequest, ExecuteResponse, ExecuteChunk, BatchExecuteRequest, CancelRequest, CancelResponse,
    StartSessionRequest, StartSessionResponse, EndSessionRequest, EndSessionResponse,
    HealthCheckRequest, HealthCheckResponse, WatchAgentsRequest, AgentStatusEvent, ShutdownRequest as ProtoShutdownRequest, ShutdownResponse as ProtoShutdownResponse,
    AgentInfo, AgentError, ExecuteError,
};
"""

content = re.sub(r'use anyhow::Result;.*?use zbus::Connection;', imports, content, flags=re.DOTALL)

# 2. Add gRPC service structs
grpc_services = """
struct GrpcAgentService {
    registry: Arc<RwLock<AgentRegistry>>,
    sessions: Arc<RwLock<HashMap<String, Vec<Arc<dyn op_agents::agents::base::AgentTrait>>>>>,
}

#[tonic::async_trait]
impl AgentLifecycle for GrpcAgentService {
    async fn start_session(&self, request: Request<StartSessionRequest>) -> Result<Response<StartSessionResponse>, Status> {
        let req = request.into_inner();
        let mut session_agents = Vec::new();
        let mut started_agents = Vec::new();
        
        let registry = self.registry.read().await;
        
        for agent_id in &req.requested_agents {
            match registry.get_agent(agent_id) {
                Some(agent) => {
                    session_agents.push(agent.clone());
                    started_agents.push(AgentInfo {
                        agent_id: agent_id.clone(),
                        agent_type: agent.name().to_string(),
                        status: 2, // Running
                        priority: 1,
                        operations: vec![],
                        started_at_ms: chrono::Utc::now().timestamp_millis(),
                    });
                }
                None => {
                    return Err(Status::not_found(format!("Agent not found: {}", agent_id)));
                }
            }
        }
        
        self.sessions.write().await.insert(req.session_id.clone(), session_agents);
        
        Ok(Response::new(StartSessionResponse {
            success: true,
            session_id: req.session_id,
            started_agents,
            failed_agents: vec![],
            started_at_ms: chrono::Utc::now().timestamp_millis(),
        }))
    }

    async fn end_session(&self, request: Request<EndSessionRequest>) -> Result<Response<EndSessionResponse>, Status> {
        let req = request.into_inner();
        self.sessions.write().await.remove(&req.session_id);
        Ok(Response::new(EndSessionResponse {
            success: true,
            stopped_agents: vec![],
            session_duration_ms: 0,
        }))
    }

    async fn health_check(&self, _request: Request<HealthCheckRequest>) -> Result<Response<HealthCheckResponse>, Status> {
        Err(Status::unimplemented("health_check"))
    }
    
    type WatchAgentsStream = tokio_stream::wrappers::ReceiverStream<Result<AgentStatusEvent, Status>>;
    async fn watch_agents(&self, _request: Request<WatchAgentsRequest>) -> Result<Response<Self::WatchAgentsStream>, Status> {
        Err(Status::unimplemented("watch_agents"))
    }
    
    async fn shutdown(&self, _request: Request<ProtoShutdownRequest>) -> Result<Response<ProtoShutdownResponse>, Status> {
        Err(Status::unimplemented("shutdown"))
    }
}

#[tonic::async_trait]
impl AgentExecution for GrpcAgentService {
    async fn execute(&self, request: Request<ExecuteRequest>) -> Result<Response<ExecuteResponse>, Status> {
        let req = request.into_inner();
        let registry = self.registry.read().await;
        
        let agent = registry.get_agent(&req.agent_id)
            .ok_or_else(|| Status::not_found(format!("Agent not found: {}", req.agent_id)))?;
            
        let task = AgentTask {
            id: req.correlation_id.clone(),
            operation: req.operation.clone(),
            arguments_json: req.arguments_json.clone(),
            context: AgentContext {
                session_id: req.session_id.clone(),
                actor_id: "grpc-client".to_string(),
                metadata: std::collections::HashMap::new(),
            },
        };
        
        match agent.execute(task).await {
            Ok(result) => {
                Ok(Response::new(ExecuteResponse {
                    correlation_id: req.correlation_id,
                    agent_id: req.agent_id,
                    operation: req.operation,
                    success: result.success,
                    result_json: result.data_json,
                    error: result.error.map(|e| ExecuteError {
                        code: "EXEC_FAILED".to_string(),
                        message: e,
                        details: "".to_string(),
                        retryable: false,
                        stack_trace: "".to_string(),
                    }),
                    execution_time_ms: 0,
                    metadata: std::collections::HashMap::new(),
                }))
            }
            Err(e) => {
                Ok(Response::new(ExecuteResponse {
                    correlation_id: req.correlation_id,
                    agent_id: req.agent_id,
                    operation: req.operation,
                    success: false,
                    result_json: "{}".to_string(),
                    error: Some(ExecuteError {
                        code: "INTERNAL".to_string(),
                        message: e.to_string(),
                        details: "".to_string(),
                        retryable: false,
                        stack_trace: "".to_string(),
                    }),
                    execution_time_ms: 0,
                    metadata: std::collections::HashMap::new(),
                }))
            }
        }
    }
    
    type ExecuteStreamStream = tokio_stream::wrappers::ReceiverStream<Result<ExecuteChunk, Status>>;
    async fn execute_stream(&self, request: Request<ExecuteRequest>) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let req = request.into_inner();
        let registry = self.registry.read().await;
        
        let agent = registry.get_agent(&req.agent_id)
            .ok_or_else(|| Status::not_found(format!("Agent not found: {}", req.agent_id)))?;
            
        let task = AgentTask {
            id: req.correlation_id.clone(),
            operation: req.operation.clone(),
            arguments_json: req.arguments_json.clone(),
            context: AgentContext {
                session_id: req.session_id.clone(),
                actor_id: "grpc-client".to_string(),
                metadata: std::collections::HashMap::new(),
            },
        };
        
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        tokio::spawn(async move {
            match agent.execute(task).await {
                Ok(result) => {
                    let _ = tx.send(Ok(ExecuteChunk {
                        correlation_id: req.correlation_id.clone(),
                        chunk_type: 4, // Result
                        content: result.data_json,
                        is_final: true,
                        sequence: 0,
                        timestamp_ms: chrono::Utc::now().timestamp_millis(),
                        error: result.error.map(|e| ExecuteError {
                            code: "EXEC_FAILED".to_string(),
                            message: e,
                            details: "".to_string(),
                            retryable: false,
                            stack_trace: "".to_string(),
                        }),
                    })).await;
                }
                Err(e) => {
                    let _ = tx.send(Ok(ExecuteChunk {
                        correlation_id: req.correlation_id.clone(),
                        chunk_type: 4, // Result
                        content: "{}".to_string(),
                        is_final: true,
                        sequence: 0,
                        timestamp_ms: chrono::Utc::now().timestamp_millis(),
                        error: Some(ExecuteError {
                            code: "INTERNAL".to_string(),
                            message: e.to_string(),
                            details: "".to_string(),
                            retryable: false,
                            stack_trace: "".to_string(),
                        }),
                    })).await;
                }
            }
        });
        
        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
    
    type BatchExecuteStream = tokio_stream::wrappers::ReceiverStream<Result<ExecuteResponse, Status>>;
    async fn batch_execute(&self, _request: Request<BatchExecuteRequest>) -> Result<Response<Self::BatchExecuteStream>, Status> {
        Err(Status::unimplemented("batch_execute"))
    }
    
    async fn cancel(&self, _request: Request<CancelRequest>) -> Result<Response<CancelResponse>, Status> {
        Err(Status::unimplemented("cancel"))
    }
}
"""

content = content.replace("struct AgentManager {", grpc_services + "\n/// Agent Manager - starts and monitors D-Bus agent services\nstruct AgentManager {")

# 3. Update main to start the gRPC server
main_additions = """    // Initialize AgentRegistry
    let mut registry = AgentRegistry::new();
    let _ = op_agents::agent_registry::AgentRegistry::load_default_specs(&registry).await;
    let registry_arc = Arc::new(RwLock::new(registry));
    
    // Start gRPC server
    let grpc_port = std::env::var("OP_AGENT_GRPC_PORT").unwrap_or_else(|_| "50055".to_string());
    let addr = format!("127.0.0.1:{}", grpc_port).parse().unwrap();
    let grpc_service = GrpcAgentService {
        registry: registry_arc.clone(),
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

"""

content = content.replace("    let mut manager = AgentManager::new(bus_type);", main_additions + "    let mut manager = AgentManager::new(bus_type);")

with open('crates/op-agents/src/bin/dbus-agent-manager.rs', 'w') as f:
    f.write(content)
