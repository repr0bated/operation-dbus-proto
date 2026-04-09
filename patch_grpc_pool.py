import re

with open('crates/op-chat/src/orchestration/grpc_pool.rs', 'r') as f:
    content = f.read()

# 1. Add tonic imports if missing
if 'use tonic::transport::Channel;' not in content:
    content = content.replace('use tokio::time::timeout;', 'use tokio::time::timeout;\nuse tonic::transport::Channel;\nuse crate::orchestration::proto::op_chat_orchestration::agent_execution_client::AgentExecutionClient;\nuse crate::orchestration::proto::op_chat_orchestration::{ExecuteRequest, ExecutionOptions};')

# 2. Add channel to AgentConnection
content = content.replace('    error_count: AtomicU64,\n    circuit_breaker: CircuitBreaker,', '    error_count: AtomicU64,\n    pub channel: Option<Channel>,\n    circuit_breaker: CircuitBreaker,')
content = content.replace('            error_count: AtomicU64::new(0),\n            circuit_breaker: CircuitBreaker::new(circuit_threshold, circuit_reset),', '            error_count: AtomicU64::new(0),\n            channel: None,\n            circuit_breaker: CircuitBreaker::new(circuit_threshold, circuit_reset),')

# 3. do_connect
old_do_connect = '''    async fn do_connect(
        &self,
        agent_id: &str,
        address: &str,
        port: u16,
    ) -> OrchestrationResult<()> {
        // TODO: Replace with actual tonic connection
        // let channel = tonic::transport::Channel::from_shared(address.to_string())?
        //     .connect_timeout(self.config.connect_timeout)
        //     .connect()
        //     .await?;

        // For now, create connection entry (simulated)
        let conn = AgentConnection::new(
            agent_id.to_string(),
            self.config.base_address.clone(),
            port,
            self.config.max_concurrent_per_agent,
            self.config.circuit_breaker_threshold,
            self.config.circuit_breaker_reset,
        );

        let mut connections = self.connections.write().await;

        if let Some(existing) = connections.get_mut(agent_id) {
            existing.connected = true;
            existing.started_at = Some(Instant::now());
        } else {
            let mut new_conn = conn;
            new_conn.connected = true;
            new_conn.started_at = Some(Instant::now());
            connections.insert(agent_id.to_string(), new_conn);
        }

        Ok(())
    }'''

new_do_connect = '''    async fn do_connect(
        &self,
        agent_id: &str,
        address: &str,
        port: u16,
    ) -> OrchestrationResult<()> {
        let uri = format!("http://{}:{}", self.config.base_address.trim_start_matches("http://").trim_start_matches("https://"), port);
        let channel = tonic::transport::Channel::from_shared(uri)
            .map_err(|e| OrchestrationError::new(ErrorCode::Configuration, format!("Invalid URI: {}", e)))?
            .connect_timeout(self.config.connect_timeout)
            .connect()
            .await
            .map_err(|e| OrchestrationError::connection_failed(format!("Failed to connect to agent {}: {}", agent_id, e)))?;

        let mut conn = AgentConnection::new(
            agent_id.to_string(),
            self.config.base_address.clone(),
            port,
            self.config.max_concurrent_per_agent,
            self.config.circuit_breaker_threshold,
            self.config.circuit_breaker_reset,
        );

        conn.channel = Some(channel);
        conn.connected = true;
        conn.started_at = Some(Instant::now());

        let mut connections = self.connections.write().await;
        if let Some(existing) = connections.get_mut(agent_id) {
            existing.channel = conn.channel;
            existing.connected = true;
            existing.started_at = Some(Instant::now());
        } else {
            connections.insert(agent_id.to_string(), conn);
        }

        Ok(())
    }'''
content = content.replace(old_do_connect, new_do_connect)

# 4. do_execute
old_do_execute = '''    async fn do_execute(
        &self,
        agent_id: &str,
        operation: &str,
        arguments: &Value,
    ) -> OrchestrationResult<Value> {
        debug!(agent = %agent_id, operation = %operation, "Executing operation");

        // Update connection stats
        {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.get_mut(agent_id) {
                conn.last_used = Some(Instant::now());
                conn.request_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        // TODO: Replace with actual gRPC call
        // let request = tonic::Request::new(ExecuteRequest {
        //     agent_id: agent_id.to_string(),
        //     operation: operation.to_string(),
        //     arguments_json: simd_json::to_string(arguments)?,
        //     timeout_ms: self.config.request_timeout.as_millis() as i64,
        //     ..Default::default()
        // });
        // let response = client.execute(request).await?;
        // let result: Value = simd_json::from_str(&response.into_inner().result_json)?;

        // Simulated successful execution
        Ok(simd_json::json!({
            "agent": agent_id,
            "operation": operation,
            "success": true,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }'''

new_do_execute = '''    async fn do_execute(
        &self,
        agent_id: &str,
        operation: &str,
        arguments: &Value,
    ) -> OrchestrationResult<Value> {
        debug!(agent = %agent_id, operation = %operation, "Executing operation");

        let channel = {
            let mut connections = self.connections.write().await;
            let conn = connections.get_mut(agent_id)
                .ok_or_else(|| OrchestrationError::agent_not_found(agent_id))?;
            conn.last_used = Some(Instant::now());
            conn.request_count.fetch_add(1, Ordering::Relaxed);
            conn.channel.clone().ok_or_else(|| OrchestrationError::connection_failed("Agent not connected"))?
        };

        let mut client = AgentExecutionClient::new(channel);
        let args_json = simd_json::to_string(arguments)
            .map_err(|e| OrchestrationError::serialization(e.to_string()))?;

        let request = ExecuteRequest {
            session_id: "pool-session".to_string(),
            agent_id: agent_id.to_string(),
            operation: operation.to_string(),
            arguments_json: args_json,
            timeout_ms: self.config.request_timeout.as_millis() as i64,
            correlation_id: format!("pool-{}", uuid::Uuid::new_v4()),
            options: Some(ExecutionOptions {
                stream_output: false,
                max_retries: self.config.max_retries as i32,
                retry_delay_ms: 1000,
                allow_partial_results: false,
                context: std::collections::HashMap::new(),
            }),
        };

        let response = client
            .execute(request)
            .await
            .map_err(|e| OrchestrationError::execution_failed(agent_id, operation, &e.to_string()))?
            .into_inner();

        if !response.success {
            let err_msg = response.error.map(|e| e.message).unwrap_or_default();
            return Err(OrchestrationError::execution_failed(agent_id, operation, &err_msg));
        }

        let mut result_json = response.result_json.into_bytes();
        let result: Value = simd_json::from_slice(&mut result_json)
            .unwrap_or(Value::Static(simd_json::StaticNode::Null));

        Ok(result)
    }'''
content = content.replace(old_do_execute, new_do_execute)

# 5. do_execute_streaming
old_do_execute_streaming = '''    async fn do_execute_streaming<F>(
        &self,
        agent_id: &str,
        operation: &str,
        arguments: &Value,
        mut on_chunk: F,
    ) -> OrchestrationResult<Value>
    where
        F: FnMut(StreamChunk) + Send + 'static,
    {
        debug!(agent = %agent_id, operation = %operation, "Starting streaming execution");

        // TODO: Replace with actual gRPC streaming call
        // let request = tonic::Request::new(ExecuteRequest { ... });
        // let mut stream = client.execute_stream(request).await?.into_inner();
        // while let Some(chunk) = stream.next().await {
        //     on_chunk(chunk.into());
        // }

        // Simulated streaming
        let mut sequence = 0u32;

        on_chunk(StreamChunk {
            content: format!("Starting {} {}...\\n", agent_id, operation),
            stream_type: StreamType::Progress,
            sequence,
            is_final: false,
            timestamp: Instant::now(),
        });
        sequence += 1;

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(100)).await;

        on_chunk(StreamChunk {
            content: format!("Executing {}...\\n", operation),
            stream_type: StreamType::Stdout,
            sequence,
            is_final: false,
            timestamp: Instant::now(),
        });
        sequence += 1;

        on_chunk(StreamChunk {
            content: "Operation complete.\\n".to_string(),
            stream_type: StreamType::Stdout,
            sequence,
            is_final: true,
            timestamp: Instant::now(),
        });

        Ok(simd_json::json!({
            "agent": agent_id,
            "operation": operation,
            "success": true,
            "streamed": true,
        }))
    }'''

new_do_execute_streaming = '''    async fn do_execute_streaming<F>(
        &self,
        agent_id: &str,
        operation: &str,
        arguments: &Value,
        mut on_chunk: F,
    ) -> OrchestrationResult<Value>
    where
        F: FnMut(StreamChunk) + Send + 'static,
    {
        debug!(agent = %agent_id, operation = %operation, "Starting streaming execution");

        let channel = {
            let mut connections = self.connections.write().await;
            let conn = connections.get_mut(agent_id)
                .ok_or_else(|| OrchestrationError::agent_not_found(agent_id))?;
            conn.last_used = Some(Instant::now());
            conn.request_count.fetch_add(1, Ordering::Relaxed);
            conn.channel.clone().ok_or_else(|| OrchestrationError::connection_failed("Agent not connected"))?
        };

        let mut client = AgentExecutionClient::new(channel);
        let args_json = simd_json::to_string(arguments)
            .map_err(|e| OrchestrationError::serialization(e.to_string()))?;

        let request = ExecuteRequest {
            session_id: "pool-session".to_string(),
            agent_id: agent_id.to_string(),
            operation: operation.to_string(),
            arguments_json: args_json,
            timeout_ms: self.config.request_timeout.as_millis() as i64,
            correlation_id: format!("pool-{}", uuid::Uuid::new_v4()),
            options: Some(ExecutionOptions {
                stream_output: true,
                max_retries: self.config.max_retries as i32,
                retry_delay_ms: 1000,
                allow_partial_results: true,
                context: std::collections::HashMap::new(),
            }),
        };

        let mut stream = client
            .execute_stream(request)
            .await
            .map_err(|e| OrchestrationError::execution_failed(agent_id, operation, &e.to_string()))?
            .into_inner();

        let mut final_result = Value::Static(simd_json::StaticNode::Null);

        use tokio_stream::StreamExt;
        let mut sequence = 0u32;
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
                        final_result = simd_json::from_slice(&mut bytes).unwrap_or(Value::Static(simd_json::StaticNode::Null));
                    }
                    
                    on_chunk(StreamChunk {
                        content: chunk.content,
                        stream_type,
                        sequence,
                        is_final: chunk.is_final,
                        timestamp: Instant::now(),
                    });
                    sequence += 1;
                }
                Err(e) => {
                    error!(error = %e, "stream error");
                    return Err(OrchestrationError::execution_failed(agent_id, operation, &e.to_string()));
                }
            }
        }

        Ok(final_result)
    }'''

content = content.replace(old_do_execute_streaming, new_do_execute_streaming)

with open('crates/op-chat/src/orchestration/grpc_pool.rs', 'w') as f:
    f.write(content)
