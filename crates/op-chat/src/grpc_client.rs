//! gRPC Agent Client
//!
//! Direct client for AgentLifecycle and AgentExecution gRPC services.

use anyhow::Context;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::{debug, error, info};

use crate::orchestration::error::{ErrorCode, OrchestrationError, OrchestrationResult};
use crate::orchestration::proto::op_chat_orchestration::{
    agent_execution_client::AgentExecutionClient, agent_lifecycle_client::AgentLifecycleClient,
    EndSessionRequest, ExecuteRequest, ExecutionOptions, StartSessionRequest,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AgentClientConfig {
    pub address: String,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_retries: u32,
}

impl Default for AgentClientConfig {
    fn default() -> Self {
        Self {
            address: std::env::var("OP_DBUS_GRPC_ADDR")
                .unwrap_or_else(|_| "http://127.0.0.1:50055".to_string()),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Active session state
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct SessionState {
    started_agents: Vec<String>,
    started_at: Option<std::time::Instant>,
}

// ---------------------------------------------------------------------------
// GrpcAgentClient
// ---------------------------------------------------------------------------

pub struct GrpcAgentClient {
    config: AgentClientConfig,
    channel: RwLock<Option<Channel>>,
    sessions: RwLock<HashMap<String, SessionState>>,
}

impl GrpcAgentClient {
    pub fn new(config: AgentClientConfig) -> Self {
        Self {
            config,
            channel: RwLock::new(None),
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(AgentClientConfig::default())
    }

    // -----------------------------------------------------------------------
    // Connect
    // -----------------------------------------------------------------------

    pub async fn connect(&self) -> OrchestrationResult<()> {
        info!(address = %self.config.address, "Connecting to agent gRPC server");

        let channel = Channel::from_shared(self.config.address.clone())
            .map_err(|e| {
                OrchestrationError::new(ErrorCode::Configuration, format!("Invalid URI: {}", e))
            })?
            .connect_timeout(self.config.connect_timeout)
            .timeout(self.config.request_timeout)
            .connect()
            .await
            .map_err(|e| {
                OrchestrationError::connection_failed(format!("Failed to connect: {}", e))
            })?;

        *self.channel.write().await = Some(channel);

        info!("Connected to agent gRPC server");
        Ok(())
    }

    fn get_channel(
        &self,
        channel_lock: &tokio::sync::RwLockReadGuard<'_, Option<Channel>>,
    ) -> OrchestrationResult<Channel> {
        channel_lock
            .as_ref()
            .cloned()
            .ok_or_else(|| OrchestrationError::connection_failed("Client not connected"))
    }

    // -----------------------------------------------------------------------
    // Session management
    // -----------------------------------------------------------------------

    pub async fn start_session(
        &self,
        session_id: &str,
        client_name: &str,
        run_on_connection: Vec<String>,
    ) -> OrchestrationResult<Vec<String>> {
        info!(session = %session_id, client = %client_name, "Starting agent session");

        let channel_lock = self.channel.read().await;
        let channel = self.get_channel(&channel_lock)?;
        let mut client = AgentLifecycleClient::new(channel);

        let request = StartSessionRequest {
            session_id: session_id.to_string(),
            client_name: client_name.to_string(),
            metadata: HashMap::new(),
            requested_agents: run_on_connection,
            timeout_ms: self.config.request_timeout.as_millis() as i64,
        };

        let response = client
            .start_session(request)
            .await
            .map_err(|e| {
                OrchestrationError::new(
                    ErrorCode::InternalError,
                    format!("StartSession failed: {}", e),
                )
            })?
            .into_inner();

        if !response.success {
            return Err(OrchestrationError::new(
                ErrorCode::InternalError,
                "Failed to start session on server",
            ));
        }

        let agents: Vec<String> = response
            .started_agents
            .into_iter()
            .map(|a| a.agent_id)
            .collect();

        self.sessions.write().await.insert(
            session_id.to_string(),
            SessionState {
                started_agents: agents.clone(),
                started_at: Some(std::time::Instant::now()),
            },
        );

        info!(session = %session_id, agents = ?agents, "Session started");
        Ok(agents)
    }

    pub async fn end_session(&self, session_id: &str) -> OrchestrationResult<()> {
        info!(session = %session_id, "Ending agent session");

        let channel_lock = self.channel.read().await;
        let channel = self.get_channel(&channel_lock)?;
        let mut client = AgentLifecycleClient::new(channel);

        let request = EndSessionRequest {
            session_id: session_id.to_string(),
            force: false,
            timeout_ms: self.config.request_timeout.as_millis() as i64,
        };

        client.end_session(request).await.map_err(|e| {
            OrchestrationError::new(
                ErrorCode::InternalError,
                format!("EndSession failed: {}", e),
            )
        })?;

        self.sessions.write().await.remove(session_id);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Dispatch
    // -----------------------------------------------------------------------

    pub async fn execute(
        &self,
        session_id: &str,
        agent_id: &str,
        operation: &str,
        arguments: Value,
    ) -> OrchestrationResult<Value> {
        debug!(
            session = %session_id,
            agent = %agent_id,
            operation = %operation,
            "Dispatching agent operation"
        );

        let channel_lock = self.channel.read().await;
        let channel = self.get_channel(&channel_lock)?;
        let mut client = AgentExecutionClient::new(channel);

        let args_json = simd_json::to_string(&arguments)
            .map_err(|e| OrchestrationError::serialization(e.to_string()))?;

        let request = ExecuteRequest {
            session_id: session_id.to_string(),
            agent_id: agent_id.to_string(),
            operation: operation.to_string(),
            arguments_json: args_json,
            timeout_ms: self.config.request_timeout.as_millis() as i64,
            correlation_id: format!("{}-{}", session_id, uuid::Uuid::new_v4()),
            options: Some(ExecutionOptions {
                stream_output: false,
                max_retries: self.config.max_retries as i32,
                retry_delay_ms: 1000,
                allow_partial_results: false,
                context: HashMap::new(),
            }),
        };

        let response = client
            .execute(request)
            .await
            .map_err(|e| OrchestrationError::execution_failed(agent_id, operation, &e.to_string()))?
            .into_inner();

        if !response.success {
            let err_msg = response.error.map(|e| e.message).unwrap_or_default();
            return Err(OrchestrationError::execution_failed(
                agent_id, operation, &err_msg,
            ));
        }

        let mut result_json = response.result_json.into_bytes();
        let result: Value = simd_json::from_slice(&mut result_json)
            .unwrap_or(Value::Static(simd_json::StaticNode::Null));

        Ok(result)
    }

    pub async fn execute_stream(
        &self,
        session_id: &str,
        agent_id: &str,
        operation: &str,
        arguments: Value,
        mut on_chunk: impl FnMut(StreamChunk) + Send + 'static,
    ) -> OrchestrationResult<Value> {
        debug!(
            session = %session_id,
            agent = %agent_id,
            operation = %operation,
            "Dispatching streaming agent operation"
        );

        let channel_lock = self.channel.read().await;
        let channel = self.get_channel(&channel_lock)?;
        let mut client = AgentExecutionClient::new(channel);

        let args_json = simd_json::to_string(&arguments)
            .map_err(|e| OrchestrationError::serialization(e.to_string()))?;

        let request = ExecuteRequest {
            session_id: session_id.to_string(),
            agent_id: agent_id.to_string(),
            operation: operation.to_string(),
            arguments_json: args_json,
            timeout_ms: self.config.request_timeout.as_millis() as i64,
            correlation_id: format!("{}-{}", session_id, uuid::Uuid::new_v4()),
            options: Some(ExecutionOptions {
                stream_output: true,
                max_retries: self.config.max_retries as i32,
                retry_delay_ms: 1000,
                allow_partial_results: true,
                context: HashMap::new(),
            }),
        };

        let mut stream = client
            .execute_stream(request)
            .await
            .map_err(|e| OrchestrationError::execution_failed(agent_id, operation, &e.to_string()))?
            .into_inner();

        let mut final_result = Value::Static(simd_json::StaticNode::Null);

        use tokio_stream::StreamExt;
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    let stream_type = match chunk.chunk_type {
                        1 => StreamType::Stdout,
                        2 => StreamType::Stderr,
                        3 => StreamType::Progress,
                        4 => StreamType::Result,
                        _ => StreamType::Stdout,
                    };

                    if chunk.chunk_type == 4 && chunk.is_final {
                        let mut bytes = chunk.content.clone().into_bytes();
                        final_result = simd_json::from_slice(&mut bytes)
                            .unwrap_or(Value::Static(simd_json::StaticNode::Null));
                    }

                    if let Some(err) = &chunk.error {
                        error!("Stream error from agent: {}", err.message);
                    }

                    on_chunk(StreamChunk {
                        content: chunk.content,
                        stream_type,
                        is_final: chunk.is_final,
                    });
                }
                Err(e) => {
                    error!(error = %e, "stream error");
                    return Err(OrchestrationError::execution_failed(
                        agent_id,
                        operation,
                        &e.to_string(),
                    ));
                }
            }
        }

        Ok(final_result)
    }

    pub async fn batch_execute(
        &self,
        session_id: &str,
        operations: Vec<(String, String, Value)>,
        parallel: bool,
    ) -> OrchestrationResult<Vec<OrchestrationResult<Value>>> {
        info!(
            session = %session_id,
            count = operations.len(),
            parallel = %parallel,
            "Batch executing agent operations"
        );

        if parallel {
            let futures: Vec<_> = operations
                .into_iter()
                .map(|(agent, op, args)| {
                    let session = session_id.to_string();
                    async move { self.execute(&session, &agent, &op, args).await }
                })
                .collect();
            Ok(futures::future::join_all(futures).await)
        } else {
            let mut results = Vec::new();
            for (agent, op, args) in operations {
                results.push(self.execute(session_id, &agent, &op, args).await);
            }
            Ok(results)
        }
    }

    // -----------------------------------------------------------------------
    // Convenience methods
    // -----------------------------------------------------------------------

    pub async fn memory_remember(
        &self,
        session_id: &str,
        key: &str,
        value: &str,
    ) -> OrchestrationResult<()> {
        self.execute(
            session_id,
            "memory",
            "remember",
            simd_json::json!({ "key": key, "value": value }),
        )
        .await?;
        Ok(())
    }

    pub async fn memory_recall(
        &self,
        session_id: &str,
        key: &str,
    ) -> OrchestrationResult<Option<String>> {
        let result = self
            .execute(
                session_id,
                "memory",
                "recall",
                simd_json::json!({ "key": key }),
            )
            .await?;
        Ok(result
            .get("value")
            .and_then(|v| v.as_str())
            .map(String::from))
    }

    pub async fn think_start(
        &self,
        session_id: &str,
        problem: &str,
        max_steps: i32,
    ) -> OrchestrationResult<String> {
        let result = self
            .execute(
                session_id,
                "sequential_thinking",
                "start_chain",
                simd_json::json!({ "problem": problem, "max_steps": max_steps }),
            )
            .await?;
        Ok(result
            .get("chain_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }

    pub async fn think(
        &self,
        session_id: &str,
        chain_id: &str,
        thought: &str,
        step: i32,
    ) -> OrchestrationResult<Value> {
        self.execute(
            session_id,
            "sequential_thinking",
            "think",
            simd_json::json!({
                "chain_id": chain_id,
                "thought": thought,
                "step": step,
            }),
        )
        .await
    }

    pub async fn cargo_check(
        &self,
        session_id: &str,
        path: &str,
        on_output: impl FnMut(StreamChunk) + Send + 'static,
    ) -> OrchestrationResult<Value> {
        self.execute_stream(
            session_id,
            "rust_pro",
            "check",
            simd_json::json!({ "path": path }),
            on_output,
        )
        .await
    }

    pub async fn cargo_build(
        &self,
        session_id: &str,
        path: &str,
        release: bool,
        on_output: impl FnMut(StreamChunk) + Send + 'static,
    ) -> OrchestrationResult<Value> {
        self.execute_stream(
            session_id,
            "rust_pro",
            "build",
            simd_json::json!({ "path": path, "release": release }),
            on_output,
        )
        .await
    }

    pub async fn context_save(
        &self,
        session_id: &str,
        name: &str,
        content: &str,
        tags: Vec<String>,
    ) -> OrchestrationResult<()> {
        self.execute(
            session_id,
            "context_manager",
            "save",
            simd_json::json!({ "name": name, "content": content, "tags": tags }),
        )
        .await?;
        Ok(())
    }

    pub async fn context_load(
        &self,
        session_id: &str,
        name: &str,
    ) -> OrchestrationResult<Option<String>> {
        let result = self
            .execute(
                session_id,
                "context_manager",
                "load",
                simd_json::json!({ "name": name }),
            )
            .await?;
        if result
            .get("found")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            Ok(result
                .get("content")
                .and_then(|v| v.as_str())
                .map(String::from))
        } else {
            Ok(None)
        }
    }

    pub async fn is_connected(&self) -> bool {
        self.channel.read().await.is_some()
    }

    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }
}

// ---------------------------------------------------------------------------
// Stream types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub stream_type: StreamType,
    pub is_final: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    Stdout,
    Stderr,
    Progress,
    Result,
}
