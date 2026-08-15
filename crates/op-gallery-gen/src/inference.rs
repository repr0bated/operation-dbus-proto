//! Inference loop for gallery generation.
//!
//! Calls the OpenAI-compatible `/v1/chat/completions` surface on op-web (:8080)
//! with Ghostbridge identity headers. (ZeroClaw's daemon on :8082 exposes A2A
//! at `/a2a/<agent>`, not this OpenAI path.)
//!
//! Tool support covers:
//! - Plugin schema queries
//! - MCP cross-blob discovery (when enabled)
//! - Qdrant semantic search (when enabled)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::context::GenerationContext;
use crate::tools::{ToolCall, ToolRegistry};

const SLED_PATH: &str = "/dev/shm/plugin_schema.dat";
const FOOTPRINT_OFFSET: usize = 40;
const TRACE_OFFSET: usize = 72;

/// Inference loop that generates UI specs.
pub struct InferenceLoop {
    /// ZeroClaw HTTP client
    client: reqwest::Client,

    /// ZeroClaw endpoint
    endpoint: String,

    /// Maximum turns per spec
    max_turns: usize,

    /// Tool registry
    tools: ToolRegistry,
}

/// OpenAI-compatible chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// OpenAI-compatible chat completion request.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub stream: bool,
}

/// OpenAI-compatible tool definition.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// OpenAI-compatible function definition.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// OpenAI-compatible chat completion response.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    #[serde(default)]
    pub id: String,
    pub choices: Vec<Choice>,
}

/// Response choice.
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

/// Generated spec result.
#[derive(Debug, Clone)]
pub struct GeneratedSpec {
    /// The spec JSON
    pub spec: serde_json::Value,

    /// Plugin targeted
    pub target_plugin: Option<String>,

    /// Generation metadata
    pub metadata: HashMap<String, String>,
}

impl InferenceLoop {
    /// Create a new inference loop.
    pub fn new(endpoint: String, max_turns: usize) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint,
            max_turns,
            tools: ToolRegistry::new(),
        }
    }

    /// Run a generation session.
    pub async fn generate(&self, ctx: &GenerationContext) -> Result<GeneratedSpec> {
        // Build initial messages
        let mut messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: Some(ctx.build_system_message()),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: Some(ctx.build_user_message()),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        // Build tool definitions
        let tool_defs = self.build_tool_definitions(ctx);

        // Inference loop
        for turn in 0..self.max_turns {
            tracing::info!("Inference turn {} of {}", turn + 1, self.max_turns);

            // Call ZeroClaw
            let response = self
                .call_zeroclaw(&messages, &tool_defs)
                .await
                .context("ZeroClaw inference call failed")?;

            // Extract assistant message
            let assistant_msg = response
                .choices
                .first()
                .map(|c| c.message.clone())
                .context("No response from model")?;

            // Check for tool calls
            if let Some(tool_calls) = &assistant_msg.tool_calls {
                // Add assistant message to history
                messages.push(assistant_msg.clone());

                // Execute tool calls
                for tool_call in tool_calls {
                    let result = self.tools.execute(tool_call, ctx).await;

                    // Add tool result to messages
                    messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content: Some(serde_json::to_string(&result)?),
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }
            } else {
                // No tool calls - check if we have a spec
                if let Some(content) = &assistant_msg.content {
                    if let Ok(spec) = self.extract_spec(content) {
                        tracing::info!("Successfully extracted spec on turn {}", turn + 1);
                        return Ok(GeneratedSpec {
                            spec,
                            target_plugin: None,
                            metadata: HashMap::new(),
                        });
                    }
                }

                // Add to history and continue
                messages.push(assistant_msg);
            }
        }

        anyhow::bail!("Max turns reached without generating a valid spec")
    }

    /// Call OpenAI-compatible chat completions (op-web → zeroclaw.Chat).
    async fn call_zeroclaw(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse> {
        let url = format!(
            "{}/v1/chat/completions",
            self.endpoint.trim_end_matches('/')
        );

        // Empty model → bridge uses ZeroClaw's selected model. Literal
        // "default" is rejected when a real model is selected.
        let request = ChatRequest {
            model: String::new(),
            messages: messages.to_vec(),
            tools: if tools.is_empty() {
                None
            } else {
                Some(tools.to_vec())
            },
            stream: false,
        };

        let mut req = self.client.post(&url).json(&request);
        if let Some((footprint, trace_id)) = live_ghostbridge_identity() {
            req = req
                .header("x-ghostbridge-footprint", footprint)
                .header("x-ghostbridge-trace-id", trace_id);
        }

        let response = req.send().await.context("HTTP request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("chat completions returned {}: {}", status, body);
        }

        response.json().await.context("Failed to parse response")
    }

    /// Build tool definitions based on enabled features.
    fn build_tool_definitions(&self, ctx: &GenerationContext) -> Vec<ToolDefinition> {
        let mut tools = vec![
            // Always available: list plugins
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_plugins".to_string(),
                    description: "List all available plugins with their names and categories"
                        .to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            // Always available: get plugin schema
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_plugin_schema".to_string(),
                    description: "Get the full schema for a specific plugin".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "plugin_name": {
                                "type": "string",
                                "description": "Name of the plugin to get schema for"
                            }
                        },
                        "required": ["plugin_name"]
                    }),
                },
            },
            // Always available: search fields
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "search_fields".to_string(),
                    description: "Search for fields across all plugins by name pattern".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description": "Pattern to search for in field names"
                            }
                        },
                        "required": ["pattern"]
                    }),
                },
            },
        ];

        // MCP-specific tools
        if ctx.mcp_enabled {
            tools.push(ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "search_methods".to_string(),
                    description: "Search for methods across all plugins by name pattern"
                        .to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description": "Pattern to search for in method names"
                            }
                        },
                        "required": ["pattern"]
                    }),
                },
            });

            tools.push(ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "search_subids".to_string(),
                    description: "Search for OSCAL subids by category or pattern".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "category": {
                                "type": "string",
                                "description": "OSCAL category to filter by (src, prj, sch, mut, obs, evt, exp)"
                            },
                            "pattern": {
                                "type": "string",
                                "description": "Pattern to search for in subids"
                            }
                        }
                    }),
                },
            });

            tools.push(ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "find_related".to_string(),
                    description:
                        "Find plugins related to a given plugin by shared fields or methods"
                            .to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "plugin_name": {
                                "type": "string",
                                "description": "Plugin to find relations for"
                            }
                        },
                        "required": ["plugin_name"]
                    }),
                },
            });
        }

        // Qdrant-specific tools
        if ctx.qdrant_enabled {
            tools.push(ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "semantic_search".to_string(),
                    description:
                        "Semantic search across all plugin schemas using vector embeddings"
                            .to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Natural language query to search for"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Maximum results to return",
                                "default": 10
                            }
                        },
                        "required": ["query"]
                    }),
                },
            });
        }

        tools
    }

    /// Assemble a spec from model output.
    ///
    /// The contract the catalog's prompt states is a JSONL patch stream, so that
    /// is what is read. A model that ignores it and emits a whole spec object is
    /// still accepted — the turn's work is usable and the gate validates it
    /// either way — but the deviation is logged, because output drifting from
    /// the prompt is worth seeing rather than absorbing.
    fn extract_spec(&self, content: &str) -> Result<serde_json::Value> {
        match crate::spec_stream::assemble(content) {
            Ok((spec, stats)) => {
                tracing::debug!(
                    "Assembled spec from {} patch operations ({} non-patch lines ignored)",
                    stats.applied,
                    stats.ignored
                );
                Ok(spec)
            }
            Err(stream_error) => {
                let whole = extract_json_object(content)
                    .filter(|value| value.get("root").is_some() && value.get("elements").is_some());
                match whole {
                    Some(spec) => {
                        tracing::warn!(
                            "Model emitted a whole spec object instead of the JSONL patch stream \
                             the catalog prompt requests; accepting it. Stream error was: {}",
                            stream_error
                        );
                        Ok(spec)
                    }
                    None => Err(stream_error),
                }
            }
        }
    }
}

/// Pull a JSON object out of model prose, bare or fenced.
fn extract_json_object(content: &str) -> Option<serde_json::Value> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(content.trim()) {
        return Some(value);
    }

    let mut rest = content;
    while let Some(fence) = rest.find("```") {
        let after_fence = &rest[fence + 3..];
        // Skip a language tag if the fence carries one.
        let body_start = after_fence.find('\n').map_or(after_fence.len(), |n| n + 1);
        let body = &after_fence[body_start..];
        let Some(end) = body.find("```") else { break };
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(body[..end].trim()) {
            return Some(value);
        }
        rest = &body[end..];
    }

    None
}

/// Resolve Ghostbridge identity for op-web `/v1/chat/completions`.
///
/// Prefers env overrides, then the live host sled at `/dev/shm/plugin_schema.dat`
/// (same offsets as `IdentitySled`: footprint @40, trace_id @72).
fn live_ghostbridge_identity() -> Option<(String, String)> {
    let env_fp = std::env::var("X_GHOSTBRIDGE_FOOTPRINT")
        .or_else(|_| std::env::var("GB_FOOTPRINT"))
        .ok()
        .filter(|v| !v.trim().is_empty());
    let env_tr = std::env::var("X_GHOSTBRIDGE_TRACE_ID")
        .or_else(|_| std::env::var("GB_TRACE_ID"))
        .ok()
        .filter(|v| !v.trim().is_empty());
    if let (Some(fp), Some(tr)) = (env_fp, env_tr) {
        return Some((fp, tr));
    }

    let bytes = std::fs::read(Path::new(SLED_PATH)).ok()?;
    if bytes.len() < TRACE_OFFSET + 16 {
        return None;
    }
    let footprint = hex::encode(&bytes[FOOTPRINT_OFFSET..FOOTPRINT_OFFSET + 32]);
    let trace_id = hex::encode(&bytes[TRACE_OFFSET..TRACE_OFFSET + 16]);
    if footprint.chars().all(|c| c == '0') || trace_id.chars().all(|c| c == '0') {
        return None;
    }
    Some((footprint, trace_id))
}
