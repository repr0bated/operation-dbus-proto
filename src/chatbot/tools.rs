//! Chatbot-specific tools.
//!
//! These tools enable the chatbot to interact with users.
//! Key tool: user.respond - ALL chatbot output goes through this.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::error::Result;
use op_execution_tracker::{ExecutionRecord, ExecutionTiming};
use op_tools::{Tool, ToolContext, ToolResult};

/// User response format
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ResponseFormat {
    Text,
    Markdown,
    Json,
    Html,
}

/// User response payload
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserRespondParams {
    pub session_id: String,
    pub message: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

/// Channel for delivering responses to web interface
pub type ResponseChannel = broadcast::Sender<UserResponseEvent>;

/// Event emitted when user.respond is executed
#[derive(Debug, Clone)]
pub struct UserResponseEvent {
    pub session_id: String,
    pub message: String,
    pub format: String,
    pub timestamp_ns: u128,
    pub execution_hash: String,
}

/// user.respond tool
///
/// CRITICAL: This is how the chatbot "speaks" to users.
/// Every chatbot response is a tool execution.
/// No descriptive responses - only tool outputs.
pub struct UserRespondTool {
    response_channel: ResponseChannel,
}

impl UserRespondTool {
    pub fn new(channel: ResponseChannel) -> Self {
        Self {
            response_channel: channel,
        }
    }
}

#[async_trait]
impl Tool for UserRespondTool {
    fn name(&self) -> &'static str {
        "user.respond"
    }

    fn input_schema(&self) -> Value {
        simd_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "Session identifier for the user"
                },
                "message": {
                    "type": "string",
                    "description": "Message content to send to user"
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "markdown", "json", "html"],
                    "default": "text",
                    "description": "Response format"
                },
                "metadata": {
                    "type": "object",
                    "description": "Additional metadata to include with response"
                }
            },
            "required": ["session_id", "message"]
        })
    }

    fn output_schema(&self) -> Value {
        simd_json::json!({
            "type": "object",
            "properties": {
                "delivered": {"type": "boolean"},
                "session_id": {"type": "string"},
                "message": {"type": "string"},
                "format": {"type": "string"},
                "timestamp_ns": {"type": "integer"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Send a response to the user. ALL chatbot output must go through this tool."
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let params: UserRespondParams = simd_json::from_value(input.clone())
            .map_err(|e| crate::error::OpDbusError::Serialization(e.to_string()))?;

        let format = params.format.unwrap_or_else(|| "text".to_string());
        let timestamp_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        // Build execution record first to get hash
        let timing = timing.complete(start);
        
        let output = simd_json::json!({
            "delivered": true,
            "session_id": params.session_id,
            "message": params.message,
            "format": format,
            "timestamp_ns": timestamp_ns,
        });

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        // Emit event to web interface
        let event = UserResponseEvent {
            session_id: params.session_id,
            message: params.message,
            format,
            timestamp_ns,
            execution_hash: record.hash.clone(),
        };

        // Best effort delivery - don't fail if no listeners
        let _ = self.response_channel.send(event);

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

/// user.prompt tool - request additional input from user
pub struct UserPromptTool {
    response_channel: ResponseChannel,
}

impl UserPromptTool {
    pub fn new(channel: ResponseChannel) -> Self {
        Self {
            response_channel: channel,
        }
    }
}

#[async_trait]
impl Tool for UserPromptTool {
    fn name(&self) -> &'static str {
        "user.prompt"
    }

    fn input_schema(&self) -> Value {
        simd_json::json!({
            "type": "object",
            "properties": {
                "session_id": {"type": "string"},
                "prompt": {"type": "string"},
                "options": {
                    "type": "array",
                    "items": {"type": "string"}
                },
                "required": {"type": "boolean", "default": true}
            },
            "required": ["session_id", "prompt"]
        })
    }

    fn output_schema(&self) -> Value {
        simd_json::json!({
            "type": "object",
            "properties": {
                "prompted": {"type": "boolean"},
                "prompt_id": {"type": "string"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Request additional input from the user"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let session_id = input.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let prompt = input.get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let prompt_id = uuid::Uuid::new_v4().to_string();

        let output = simd_json::json!({
            "prompted": true,
            "prompt_id": prompt_id,
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        // Emit as response event
        let event = UserResponseEvent {
            session_id: session_id.to_string(),
            message: format!("[PROMPT:{}] {}", prompt_id, prompt),
            format: "prompt".to_string(),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            execution_hash: record.hash.clone(),
        };

        let _ = self.response_channel.send(event);

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

/// user.stream tool - stream partial responses
pub struct UserStreamTool {
    response_channel: ResponseChannel,
}

impl UserStreamTool {
    pub fn new(channel: ResponseChannel) -> Self {
        Self {
            response_channel: channel,
        }
    }
}

#[async_trait]
impl Tool for UserStreamTool {
    fn name(&self) -> &'static str {
        "user.stream"
    }

    fn input_schema(&self) -> Value {
        simd_json::json!({
            "type": "object",
            "properties": {
                "session_id": {"type": "string"},
                "chunk": {"type": "string"},
                "stream_id": {"type": "string"},
                "is_final": {"type": "boolean", "default": false}
            },
            "required": ["session_id", "chunk"]
        })
    }

    fn output_schema(&self) -> Value {
        simd_json::json!({
            "type": "object",
            "properties": {
                "streamed": {"type": "boolean"},
                "stream_id": {"type": "string"},
                "is_final": {"type": "boolean"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Stream partial response chunks to the user"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let session_id = input.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let chunk = input.get("chunk")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let stream_id = input.get("stream_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let is_final = input.get("is_final")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let output = simd_json::json!({
            "streamed": true,
            "stream_id": stream_id,
            "is_final": is_final,
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        // Emit stream event
        let event = UserResponseEvent {
            session_id: session_id.to_string(),
            message: chunk.to_string(),
            format: if is_final { "stream_end" } else { "stream" }.to_string(),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            execution_hash: record.hash.clone(),
        };

        let _ = self.response_channel.send(event);

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

/// Register all chatbot tools
pub fn register_chatbot_tools(
    registry: &op_tools::ToolRegistry,
    response_channel: ResponseChannel,
) -> Result<()> {
    registry.register(
        Arc::new(UserRespondTool::new(response_channel.clone())),
        "chatbot",
        "1.0.0",
    )?;
    
    registry.register(
        Arc::new(UserPromptTool::new(response_channel.clone())),
        "chatbot",
        "1.0.0",
    )?;
    
    registry.register(
        Arc::new(UserStreamTool::new(response_channel)),
        "chatbot",
        "1.0.0",
    )?;

    Ok(())
}
