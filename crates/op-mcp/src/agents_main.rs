//! MCP Agents Server - Stdio Transport
//!
//! This binary provides a stdio-based MCP server that exposes agent tools
//! for use with clients like Gemini CLI that only support stdio transport.
//!
//! Usage:
//!   op-mcp-agents-server
//!
//! The server reads JSON-RPC requests from stdin and writes responses to stdout.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tokio::sync::RwLock;

/// MCP Protocol version
const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "op-mcp-agents-server";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// JSON-RPC Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

// ============================================================================
// MCP Types
// ============================================================================

#[derive(Debug, Serialize)]
struct ServerInfo {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct ServerCapabilities {
    tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
struct ToolsCapability {
    #[serde(rename = "listChanged")]
    list_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

// ============================================================================
// Agent Tool Definitions
// ============================================================================

fn get_agent_tools() -> Vec<ToolDefinition> {
    vec![
        // Sequential Thinking Agent
        ToolDefinition {
            name: "agent_sequential_thinking".to_string(),
            description: "A detailed thinking tool that helps break down complex problems into sequential steps. Use this for multi-step reasoning, planning, and analysis.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "thought": {
                        "type": "string",
                        "description": "The current thought or reasoning step"
                    },
                    "operation": {
                        "type": "string",
                        "description": "Operation to perform: think, plan, analyze, conclude",
                        "enum": ["think", "plan", "analyze", "conclude"]
                    },
                    "context": {
                        "type": "string",
                        "description": "Additional context for the thinking process"
                    }
                }
            }),
        },
        // Memory Agent
        ToolDefinition {
            name: "agent_memory".to_string(),
            description: "Store and retrieve information from conversation memory. Use for maintaining context across interactions.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: store, retrieve, search, clear",
                        "enum": ["store", "retrieve", "search", "clear"]
                    },
                    "key": {
                        "type": "string",
                        "description": "Key for storing/retrieving data"
                    },
                    "value": {
                        "type": "string",
                        "description": "Value to store (for store operation)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query (for search operation)"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Code Review Agent
        ToolDefinition {
            name: "agent_code_review".to_string(),
            description: "Analyze code for issues, suggest improvements, and provide review feedback.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: review, suggest, analyze_security, check_style",
                        "enum": ["review", "suggest", "analyze_security", "check_style"]
                    },
                    "code": {
                        "type": "string",
                        "description": "Code to review"
                    },
                    "language": {
                        "type": "string",
                        "description": "Programming language"
                    },
                    "focus": {
                        "type": "string",
                        "description": "Specific area to focus on"
                    }
                },
                "required": ["operation", "code"]
            }),
        },
        // Rust Expert Agent
        ToolDefinition {
            name: "agent_rust_pro".to_string(),
            description: "Expert Rust programming assistance. Helps with Rust code, best practices, error handling, and optimization.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: help, review, optimize, explain, fix_error",
                        "enum": ["help", "review", "optimize", "explain", "fix_error"]
                    },
                    "code": {
                        "type": "string",
                        "description": "Rust code to analyze"
                    },
                    "error": {
                        "type": "string",
                        "description": "Error message to help fix"
                    },
                    "question": {
                        "type": "string",
                        "description": "Question about Rust"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Python Expert Agent
        ToolDefinition {
            name: "agent_python_pro".to_string(),
            description: "Expert Python programming assistance. Helps with Python code, best practices, and optimization.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: help, review, optimize, explain, fix_error",
                        "enum": ["help", "review", "optimize", "explain", "fix_error"]
                    },
                    "code": {
                        "type": "string",
                        "description": "Python code to analyze"
                    },
                    "error": {
                        "type": "string",
                        "description": "Error message to help fix"
                    },
                    "question": {
                        "type": "string",
                        "description": "Question about Python"
                    }
                },
                "required": ["operation"]
            }),
        },
        // DevOps Troubleshooter Agent
        ToolDefinition {
            name: "agent_devops_troubleshooter".to_string(),
            description: "DevOps and infrastructure troubleshooting. Helps diagnose and fix deployment, container, and infrastructure issues.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: diagnose, suggest_fix, analyze_logs, check_config",
                        "enum": ["diagnose", "suggest_fix", "analyze_logs", "check_config"]
                    },
                    "issue": {
                        "type": "string",
                        "description": "Description of the issue"
                    },
                    "logs": {
                        "type": "string",
                        "description": "Relevant log output"
                    },
                    "config": {
                        "type": "string",
                        "description": "Configuration to analyze"
                    },
                    "context": {
                        "type": "string",
                        "description": "Additional context (k8s, docker, systemd, etc.)"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Network Expert Agent
        ToolDefinition {
            name: "agent_network_expert".to_string(),
            description: "Network configuration and troubleshooting expert. Helps with OVS, routing, firewalls, and network debugging.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: diagnose, configure, explain, troubleshoot",
                        "enum": ["diagnose", "configure", "explain", "troubleshoot"]
                    },
                    "issue": {
                        "type": "string",
                        "description": "Network issue or question"
                    },
                    "topology": {
                        "type": "string",
                        "description": "Network topology description"
                    },
                    "config": {
                        "type": "string",
                        "description": "Current network configuration"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Database Architect Agent
        ToolDefinition {
            name: "agent_database_architect".to_string(),
            description: "Database design, optimization, and troubleshooting. Supports SQL, PostgreSQL, Redis, and more.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: design, optimize, review, migrate, troubleshoot",
                        "enum": ["design", "optimize", "review", "migrate", "troubleshoot"]
                    },
                    "query": {
                        "type": "string",
                        "description": "SQL query to analyze"
                    },
                    "schema": {
                        "type": "string",
                        "description": "Database schema"
                    },
                    "requirements": {
                        "type": "string",
                        "description": "Requirements for design"
                    },
                    "database_type": {
                        "type": "string",
                        "description": "Database type (postgres, mysql, redis, etc.)"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Security Auditor Agent
        ToolDefinition {
            name: "agent_security_auditor".to_string(),
            description: "Security analysis and auditing. Reviews code, configs, and infrastructure for vulnerabilities.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: audit, scan, review, recommend",
                        "enum": ["audit", "scan", "review", "recommend"]
                    },
                    "target": {
                        "type": "string",
                        "description": "Code, config, or system to audit"
                    },
                    "target_type": {
                        "type": "string",
                        "description": "Type: code, config, infrastructure, api"
                    },
                    "focus": {
                        "type": "string",
                        "description": "Specific security area to focus on"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Kubernetes Expert Agent
        ToolDefinition {
            name: "agent_kubernetes_expert".to_string(),
            description: "Kubernetes configuration, deployment, and troubleshooting expert.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: deploy, diagnose, optimize, explain, generate",
                        "enum": ["deploy", "diagnose", "optimize", "explain", "generate"]
                    },
                    "manifest": {
                        "type": "string",
                        "description": "Kubernetes manifest YAML"
                    },
                    "issue": {
                        "type": "string",
                        "description": "Issue or question"
                    },
                    "requirements": {
                        "type": "string",
                        "description": "Deployment requirements"
                    }
                },
                "required": ["operation"]
            }),
        },
    ]
}

// ============================================================================
// Agent Execution
// ============================================================================

struct AgentServer {
    memory: Arc<RwLock<HashMap<String, String>>>,
    thinking_history: Arc<RwLock<Vec<String>>>,
}

impl AgentServer {
    fn new() -> Self {
        Self {
            memory: Arc::new(RwLock::new(HashMap::new())),
            thinking_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn execute_tool(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            "agent_sequential_thinking" => self.sequential_thinking(args).await,
            "agent_memory" => self.memory_operations(args).await,
            "agent_code_review" => self.code_review(args).await,
            "agent_rust_pro" => self.language_expert("Rust", args).await,
            "agent_python_pro" => self.language_expert("Python", args).await,
            "agent_devops_troubleshooter" => self.devops_troubleshoot(args).await,
            "agent_network_expert" => self.network_expert(args).await,
            "agent_database_architect" => self.database_architect(args).await,
            "agent_security_auditor" => self.security_auditor(args).await,
            "agent_kubernetes_expert" => self.kubernetes_expert(args).await,
            _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }
    }

    async fn sequential_thinking(&self, args: Value) -> Result<Value> {
        // Accept either "thought" or "operation" field
        let thought = args
            .as_object()
            .and_then(|o| o.get("thought"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                args.as_object()
                    .and_then(|o| o.get("operation"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("No thought provided");

        let context = args
            .as_object()
            .and_then(|o| o.get("context"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Store in thinking history
        {
            let mut history = self.thinking_history.write().await;
            history.push(thought.to_string());
        }

        let step_number = self.thinking_history.read().await.len();

        Ok(json!({
            "status": "success",
            "step": step_number,
            "thought": thought,
            "context": context,
            "message": format!("Thinking step {} recorded: {}", step_number,
                if thought.len() > 50 { format!("{}...", &thought[..50]) } else { thought.to_string() })
        }))
    }

    async fn memory_operations(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;

        match operation {
            "store" => {
                let key = args
                    .as_object()
                    .and_then(|o| o.get("key"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing key"))?;
                let value = args
                    .as_object()
                    .and_then(|o| o.get("value"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing value"))?;

                self.memory
                    .write()
                    .await
                    .insert(key.to_string(), value.to_string());
                Ok(json!({ "status": "stored", "key": key }))
            }
            "retrieve" => {
                let key = args
                    .as_object()
                    .and_then(|o| o.get("key"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing key"))?;

                let value = self.memory.read().await.get(key).cloned();
                Ok(json!({ "status": "retrieved", "key": key, "value": value }))
            }
            "search" => {
                let query = args
                    .as_object()
                    .and_then(|o| o.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let memory = self.memory.read().await;
                let matches: Vec<_> = memory
                    .iter()
                    .filter(|(k, v)| k.contains(query) || v.contains(query))
                    .map(|(k, v)| json!({ "key": k, "value": v }))
                    .collect();

                Ok(json!({ "status": "searched", "matches": matches, "count": matches.len() }))
            }
            "clear" => {
                self.memory.write().await.clear();
                Ok(json!({ "status": "cleared" }))
            }
            _ => Err(anyhow::anyhow!("Unknown memory operation: {}", operation)),
        }
    }

    async fn code_review(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;
        let code = args
            .as_object()
            .and_then(|o| o.get("code"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing code"))?;
        let language = args
            .as_object()
            .and_then(|o| o.get("language"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(json!({
            "status": "success",
            "operation": operation,
            "language": language,
            "code_length": code.len(),
            "analysis": format!("Code review ({}) for {} code ({} chars). Use an LLM to get detailed analysis.",
                operation, language, code.len()),
            "note": "This agent provides structure for code review. Connect to an LLM for detailed analysis."
        }))
    }

    async fn language_expert(&self, language: &str, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;

        Ok(json!({
            "status": "success",
            "language": language,
            "operation": operation,
            "message": format!("{} expert ready for {} operation", language, operation),
            "note": "This agent provides structure for language-specific help. Connect to an LLM for detailed assistance."
        }))
    }

    async fn devops_troubleshoot(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;
        let issue = args
            .as_object()
            .and_then(|o| o.get("issue"))
            .and_then(|v| v.as_str())
            .unwrap_or("No issue specified");

        Ok(json!({
            "status": "success",
            "operation": operation,
            "issue": issue,
            "message": format!("DevOps troubleshooter analyzing: {}",
                if issue.len() > 50 { format!("{}...", &issue[..50]) } else { issue.to_string() }),
            "note": "This agent provides structure for DevOps troubleshooting. Connect to an LLM for detailed diagnosis."
        }))
    }

    async fn network_expert(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;

        Ok(json!({
            "status": "success",
            "operation": operation,
            "message": format!("Network expert ready for {} operation", operation),
            "capabilities": ["OVS", "routing", "firewall", "DNS", "VPN", "troubleshooting"],
            "note": "This agent provides structure for network expertise. Connect to an LLM for detailed help."
        }))
    }

    async fn database_architect(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;
        let db_type = args
            .as_object()
            .and_then(|o| o.get("database_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("generic");

        Ok(json!({
            "status": "success",
            "operation": operation,
            "database_type": db_type,
            "message": format!("Database architect ready for {} on {}", operation, db_type),
            "note": "This agent provides structure for database architecture. Connect to an LLM for detailed help."
        }))
    }

    async fn security_auditor(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;
        let target_type = args
            .as_object()
            .and_then(|o| o.get("target_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(json!({
            "status": "success",
            "operation": operation,
            "target_type": target_type,
            "message": format!("Security auditor ready for {} on {}", operation, target_type),
            "note": "This agent provides structure for security auditing. Connect to an LLM for detailed analysis."
        }))
    }

    async fn kubernetes_expert(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;

        Ok(json!({
            "status": "success",
            "operation": operation,
            "message": format!("Kubernetes expert ready for {} operation", operation),
            "capabilities": ["deployment", "services", "ingress", "configmaps", "secrets", "troubleshooting"],
            "note": "This agent provides structure for Kubernetes help. Connect to an LLM for detailed assistance."
        }))
    }
}

// ============================================================================
// MCP Protocol Handler
// ============================================================================

async fn handle_request(server: &AgentServer, request: JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            request.id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": { "listChanged": false }
                },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION
                }
            }),
        ),
        "initialized" => {
            // Notification, no response needed
            JsonRpcResponse::success(request.id, json!({}))
        }
        "tools/list" => {
            let tools = get_agent_tools();
            JsonRpcResponse::success(
                request.id,
                json!({
                    "tools": tools
                }),
            )
        }
        "tools/call" => {
            let tool_name = request
                .params
                .as_object()
                .and_then(|o| o.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = request
                .params
                .as_object()
                .and_then(|o| o.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            match server.execute_tool(tool_name, arguments).await {
                Ok(result) => JsonRpcResponse::success(
                    request.id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": simd_json::to_string_pretty(&result).unwrap_or_default()
                        }],
                        "isError": false
                    }),
                ),
                Err(e) => JsonRpcResponse::success(
                    request.id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Error: {}", e)
                        }],
                        "isError": true
                    }),
                ),
            }
        }
        "ping" => JsonRpcResponse::success(request.id, json!({})),
        _ => JsonRpcResponse::error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Set up stderr logging (stdout is for JSON-RPC)
    eprintln!(
        "[{}] Starting {} v{}",
        SERVER_NAME, SERVER_NAME, SERVER_VERSION
    );

    let server = AgentServer::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let mut line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[{}] Error reading stdin: {}", SERVER_NAME, e);
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        // Parse JSON-RPC request
        let request: JsonRpcRequest = match unsafe { simd_json::from_str(&mut line) } {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[{}] Parse error: {}", SERVER_NAME, e);
                let error_response =
                    JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                let _ = writeln!(stdout, "{}", simd_json::to_string(&error_response).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        eprintln!("[{}] Received: {}", SERVER_NAME, request.method);

        // Handle request
        let response = handle_request(&server, request).await;

        // Write response
        if let Err(e) = writeln!(stdout, "{}", simd_json::to_string(&response).unwrap()) {
            eprintln!("[{}] Error writing response: {}", SERVER_NAME, e);
            break;
        }
        let _ = stdout.flush();
    }

    eprintln!("[{}] Shutting down", SERVER_NAME);
    Ok(())
}
