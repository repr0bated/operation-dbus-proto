//! Live MCP mode for cognitive streaming.
//!
//! Live MCP enables:
//! - Rapid access to cognitive agents
//! - Streaming intermediate reasoning
//! - Agent collaboration coordination
//!
//! All execution still goes through the orchestrator.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use tokio::sync::{broadcast, mpsc, RwLock};

use super::cognitive::ReasoningTrace;

/// Cognitive agent types
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveAgentType {
    /// Planner - decomposes complex requests
    Planner,
    /// Executor - manages tool execution
    Executor,
    /// Validator - verifies outputs
    Validator,
    /// Explainer - generates explanations
    Explainer,
    /// Monitor - tracks system state
    Monitor,
}

/// Cognitive agent
#[derive(Debug, Clone)]
pub struct CognitiveAgent {
    pub agent_type: CognitiveAgentType,
    pub agent_id: String,
    pub capabilities: Vec<String>,
}

impl CognitiveAgent {
    pub fn new(agent_type: CognitiveAgentType) -> Self {
        Self {
            agent_id: uuid::Uuid::new_v4().to_string(),
            capabilities: match agent_type {
                CognitiveAgentType::Planner => vec![
                    "decompose".into(),
                    "sequence".into(),
                    "parallelize".into(),
                ],
                CognitiveAgentType::Executor => vec![
                    "dispatch".into(),
                    "retry".into(),
                    "rollback".into(),
                ],
                CognitiveAgentType::Validator => vec![
                    "validate_output".into(),
                    "check_invariants".into(),
                ],
                CognitiveAgentType::Explainer => vec![
                    "summarize".into(),
                    "detail".into(),
                ],
                CognitiveAgentType::Monitor => vec![
                    "observe".into(),
                    "alert".into(),
                ],
            },
            agent_type,
        }
    }
}

/// Message between cognitive agents
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentMessage {
    pub from_agent: String,
    pub to_agent: Option<String>, // None = broadcast
    pub message_type: AgentMessageType,
    pub payload: Value,
    pub timestamp_ns: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageType {
    /// Request to perform action
    Request,
    /// Response to request
    Response,
    /// Intermediate reasoning update
    Reasoning,
    /// Execution status update
    Status,
    /// Error notification
    Error,
    /// Coordination signal
    Coordination,
}

/// Live MCP stream event
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LiveMcpEvent {
    pub event_type: LiveMcpEventType,
    pub session_id: String,
    pub payload: Value,
    pub timestamp_ns: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LiveMcpEventType {
    /// Reasoning step occurred
    Reasoning,
    /// Tool about to be executed
    ToolPending,
    /// Tool execution completed
    ToolComplete,
    /// Agent message
    AgentMessage,
    /// Stream started
    StreamStart,
    /// Stream ended
    StreamEnd,
}

/// Live MCP stream for cognitive functions
pub struct LiveMcpStream {
    /// Event broadcast channel
    event_tx: broadcast::Sender<LiveMcpEvent>,
    /// Registered cognitive agents
    agents: RwLock<Vec<CognitiveAgent>>,
    /// Agent message channel
    agent_tx: mpsc::Sender<AgentMessage>,
    agent_rx: RwLock<Option<mpsc::Receiver<AgentMessage>>>,
}

impl LiveMcpStream {
    pub fn new(buffer_size: usize) -> Self {
        let (event_tx, _) = broadcast::channel(buffer_size);
        let (agent_tx, agent_rx) = mpsc::channel(buffer_size);

        Self {
            event_tx,
            agents: RwLock::new(Vec::new()),
            agent_tx,
            agent_rx: RwLock::new(Some(agent_rx)),
        }
    }

    /// Subscribe to live events
    pub fn subscribe(&self) -> broadcast::Receiver<LiveMcpEvent> {
        self.event_tx.subscribe()
    }

    /// Emit a reasoning trace
    pub async fn emit_reasoning(&self, result: &super::cognitive::CognitiveResult) {
        let event = LiveMcpEvent {
            event_type: LiveMcpEventType::Reasoning,
            session_id: "system".to_string(),
            payload: simd_json::json!({
                "intent": result.intent,
                "tools_mapped": result.tool_mappings.len(),
                "confidence": result.reasoning_trace.confidence,
            }),
            timestamp_ns: now_ns(),
        };
        let _ = self.event_tx.send(event);
    }

    /// Emit tool pending event
    pub async fn emit_tool_pending(&self, session_id: &str, tool_name: &str, params: &Value) {
        let event = LiveMcpEvent {
            event_type: LiveMcpEventType::ToolPending,
            session_id: session_id.to_string(),
            payload: simd_json::json!({
                "tool": tool_name,
                "params": params,
            }),
            timestamp_ns: now_ns(),
        };
        let _ = self.event_tx.send(event);
    }

    /// Emit tool complete event
    pub async fn emit_tool_complete(
        &self,
        session_id: &str,
        tool_name: &str,
        output: &Value,
        execution_hash: &str,
    ) {
        let event = LiveMcpEvent {
            event_type: LiveMcpEventType::ToolComplete,
            session_id: session_id.to_string(),
            payload: simd_json::json!({
                "tool": tool_name,
                "output": output,
                "execution_hash": execution_hash,
            }),
            timestamp_ns: now_ns(),
        };
        let _ = self.event_tx.send(event);
    }

    /// Register a cognitive agent
    pub async fn register_agent(&self, agent: CognitiveAgent) {
        let mut agents = self.agents.write().await;
        agents.push(agent);
    }

    /// Send message between agents
    pub async fn send_agent_message(&self, message: AgentMessage) {
        let _ = self.agent_tx.send(message).await;
        
        // Also emit as live event
        let event = LiveMcpEvent {
            event_type: LiveMcpEventType::AgentMessage,
            session_id: "agents".to_string(),
            payload: simd_json::json!({
                "from": message.from_agent,
                "to": message.to_agent,
                "type": message.message_type,
            }),
            timestamp_ns: now_ns(),
        };
        let _ = self.event_tx.send(event);
    }

    /// Get registered agents
    pub async fn list_agents(&self) -> Vec<CognitiveAgent> {
        self.agents.read().await.clone()
    }

    /// Start a streaming session
    pub async fn start_stream(&self, session_id: &str) {
        let event = LiveMcpEvent {
            event_type: LiveMcpEventType::StreamStart,
            session_id: session_id.to_string(),
            payload: simd_json::json!({}),
            timestamp_ns: now_ns(),
        };
        let _ = self.event_tx.send(event);
    }

    /// End a streaming session
    pub async fn end_stream(&self, session_id: &str) {
        let event = LiveMcpEvent {
            event_type: LiveMcpEventType::StreamEnd,
            session_id: session_id.to_string(),
            payload: simd_json::json!({}),
            timestamp_ns: now_ns(),
        };
        let _ = self.event_tx.send(event);
    }
}

fn now_ns() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
