//! MCP Server implementation for Chat Orchestration (gRPC)
//!
//! Exposes chat capabilities as an MCP server over gRPC.
//! Uses the generic `call` tunnel to handle JSON-RPC requests for Prompts and Resources.

use crate::orchestration::builtin_workstacks;
use crate::ChatActor;
use anyhow::Result;
use op_mcp::grpc::proto::mcp_service_server::{McpService, McpServiceServer};
use op_mcp::grpc::proto::{
    CallToolRequest, CallToolResponse, GetToolSchemaRequest, GetToolSchemaResponse, HealthRequest,
    HealthResponse, InitializeRequest, InitializeResponse, ListToolsRequest, ListToolsResponse,
    McpError as ProtoMcpError, McpEvent as ProtoMcpEvent, McpRequest as ProtoMcpRequest,
    McpResponse as ProtoMcpResponse, SubscribeRequest, ToolOutput as ProtoToolOutput,
};
use op_mcp::protocol::{JsonRpcError, McpRequest, McpResponse};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

// ============================================================================
// LOCALLY DEFINED MCP TYPES
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct Prompt {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<Vec<PromptArgument>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PromptArgument {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Resource {
    uri: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
}

// ============================================================================
// PROTO <-> JSON CONVERSION HELPERS
// ============================================================================

/// Convert prost_types::Struct to simd_json::OwnedValue
fn struct_to_value(s: &prost_types::Struct) -> Option<Value> {
    // Convert prost Struct fields into a JSON-like map
    let mut map = simd_json::owned::Object::new();
    for (key, val) in &s.fields {
        if let Some(ref kind) = val.kind {
            let json_val = prost_value_to_json(kind);
            map.insert(key.clone(), json_val);
        }
    }
    Some(Value::Object(Box::new(map)))
}

/// Convert a prost Value kind to simd_json Value
fn prost_value_to_json(kind: &prost_types::value::Kind) -> Value {
    match kind {
        prost_types::value::Kind::NullValue(_) => Value::Static(simd_json::StaticNode::Null),
        prost_types::value::Kind::NumberValue(n) => {
            simd_json::json!(*n)
        }
        prost_types::value::Kind::StringValue(s) => Value::String(s.clone()),
        prost_types::value::Kind::BoolValue(b) => Value::Static(simd_json::StaticNode::Bool(*b)),
        prost_types::value::Kind::StructValue(s) => struct_to_value(s).unwrap_or_else(|| json!({})),
        prost_types::value::Kind::ListValue(list) => {
            let items: Vec<Value> = list
                .values
                .iter()
                .filter_map(|v| v.kind.as_ref().map(prost_value_to_json))
                .collect();
            Value::Array(items)
        }
    }
}

/// Convert simd_json::OwnedValue to prost_types::Struct
fn value_to_struct(v: &Value) -> Option<prost_types::Struct> {
    // For simplicity, serialize to JSON string then use prost's JSON mapping
    // prost_types::Struct implements Serialize/Deserialize via prost
    match v {
        Value::Object(obj) => {
            let mut fields = std::collections::BTreeMap::new();
            for (k, v) in obj.iter() {
                fields.insert(k.clone(), json_to_prost_value(v));
            }
            Some(prost_types::Struct { fields })
        }
        _ => None,
    }
}

/// Convert simd_json Value to prost Value
fn json_to_prost_value(v: &Value) -> prost_types::Value {
    let kind = match v {
        Value::Static(simd_json::StaticNode::Null) => prost_types::value::Kind::NullValue(0),
        Value::Static(simd_json::StaticNode::Bool(b)) => prost_types::value::Kind::BoolValue(*b),
        Value::Static(simd_json::StaticNode::I64(n)) => {
            prost_types::value::Kind::NumberValue(*n as f64)
        }
        Value::Static(simd_json::StaticNode::U64(n)) => {
            prost_types::value::Kind::NumberValue(*n as f64)
        }
        Value::Static(simd_json::StaticNode::F64(n)) => prost_types::value::Kind::NumberValue(*n),
        Value::String(s) => prost_types::value::Kind::StringValue(s.clone()),
        Value::Array(arr) => {
            let values = arr.iter().map(json_to_prost_value).collect();
            prost_types::value::Kind::ListValue(prost_types::ListValue { values })
        }
        Value::Object(_) => match value_to_struct(v) {
            Some(s) => prost_types::value::Kind::StructValue(s),
            None => prost_types::value::Kind::NullValue(0),
        },
    };
    prost_types::Value { kind: Some(kind) }
}

// ============================================================================
// SERVER IMPLEMENTATION
// ============================================================================

pub struct ChatMcpServer {
    chat_actor: Arc<ChatActor>,
}

impl ChatMcpServer {
    pub fn new(chat_actor: Arc<ChatActor>) -> Self {
        Self { chat_actor }
    }

    async fn handle_internal_request(&self, req: McpRequest) -> McpResponse {
        match req.method.as_str() {
            "prompts/list" => self.handle_list_prompts(req.id).await,
            "prompts/get" => self.handle_get_prompt(req.id, req.params).await,
            "resources/list" => self.handle_list_resources(req.id).await,
            "resources/read" => self.handle_read_resource(req.id, req.params).await,
            "tools/list" => self.handle_list_tools_internal(req.id).await,
            "tools/call" => self.handle_call_tool_internal(req.id, req.params).await,
            _ => McpResponse::error(req.id, JsonRpcError::method_not_found(&req.method)),
        }
    }

    async fn handle_list_prompts(&self, id: Option<Value>) -> McpResponse {
        let workstacks = builtin_workstacks();
        let prompts: Vec<Prompt> = workstacks
            .into_iter()
            .map(|ws| Prompt {
                name: ws.id,
                description: Some(ws.description),
                arguments: Some(vec![PromptArgument {
                    name: "context".to_string(),
                    description: Some("Context variables".to_string()),
                    required: Some(false),
                }]),
            })
            .collect();

        McpResponse::success(id, json!({ "prompts": prompts }))
    }

    async fn handle_get_prompt(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let name = params
            .as_ref()
            .and_then(|p| p.get("name"))
            .and_then(|v: &Value| v.as_str());

        if let Some(name) = name {
            if let Some(ws) = builtin_workstacks().into_iter().find(|w| w.id == name) {
                let prompt_text = format!(
                    "Execute Workstack: {}\nDescription: {}",
                    ws.name, ws.description
                );

                return McpResponse::success(
                    id,
                    json!({
                        "description": ws.description,
                        "messages": [{
                            "role": "user",
                            "content": {
                                "type": "text",
                                "text": prompt_text
                            }
                        }]
                    }),
                );
            }
        }
        McpResponse::error(id, JsonRpcError::invalid_params("Prompt not found"))
    }

    async fn handle_list_resources(&self, id: Option<Value>) -> McpResponse {
        let resources = vec![Resource {
            uri: "chat://sessions/active".to_string(),
            name: "Active Sessions".to_string(),
            description: Some("List of active sessions".to_string()),
            mime_type: Some("application/json".to_string()),
        }];
        McpResponse::success(id, json!({ "resources": resources }))
    }

    async fn handle_read_resource(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let uri = params
            .as_ref()
            .and_then(|p| p.get("uri"))
            .and_then(|v: &Value| v.as_str());

        if uri == Some("chat://sessions/active") {
            let sessions = self.chat_actor.session_manager().list_sessions().await;
            let content = json!(sessions);
            return McpResponse::success(
                id,
                json!({
                    "contents": [{
                        "uri": "chat://sessions/active",
                        "mimeType": "application/json",
                        "text": content.to_string()
                    }]
                }),
            );
        }

        McpResponse::error(id, JsonRpcError::invalid_params("Resource not found"))
    }

    async fn handle_list_tools_internal(&self, id: Option<Value>) -> McpResponse {
        let tools = self.chat_actor.tool_registry().list().await;
        let tool_list: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                })
            })
            .collect();

        McpResponse::success(id, json!({ "tools": tool_list }))
    }

    async fn handle_call_tool_internal(
        &self,
        id: Option<Value>,
        params: Option<Value>,
    ) -> McpResponse {
        let (name, arguments) = match params.as_ref() {
            Some(p) => {
                let name = p.get("name").and_then(|v: &Value| v.as_str());
                let args = p.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match name {
                    Some(n) => (n.to_string(), args),
                    None => {
                        return McpResponse::error(
                            id,
                            JsonRpcError::invalid_params("Missing 'name' parameter"),
                        )
                    }
                }
            }
            None => return McpResponse::error(id, JsonRpcError::invalid_params("Missing params")),
        };

        match self
            .chat_actor
            .tool_executor()
            .execute(&name, arguments, None)
            .await
        {
            Ok(tracked) => {
                let result = tracked.result.result.clone().unwrap_or_else(|| json!(null));
                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": simd_json::to_string(&result).unwrap_or_default()
                        }],
                        "isError": !tracked.success()
                    }),
                )
            }
            Err(e) => McpResponse::success(
                id,
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
}

// ============================================================================
// TONIC gRPC IMPLEMENTATION
// ============================================================================

#[tonic::async_trait]
impl McpService for ChatMcpServer {
    async fn call(
        &self,
        request: Request<ProtoMcpRequest>,
    ) -> std::result::Result<Response<ProtoMcpResponse>, Status> {
        let req = request.into_inner();

        // Convert proto params to internal Value
        let params = req.params.as_ref().and_then(struct_to_value);

        let internal_req = McpRequest {
            jsonrpc: "2.0".to_string(),
            method: req.method,
            id: req.id.as_ref().map(|v: &String| json!(v.clone())),
            params,
        };

        let internal_resp = self.handle_internal_request(internal_req).await;

        // Convert internal result Value to proto Struct
        let result = internal_resp.result.as_ref().and_then(value_to_struct);

        Ok(Response::new(ProtoMcpResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result,
            error: internal_resp.error.map(|e| ProtoMcpError {
                code: e.code,
                message: e.message,
                data: None,
            }),
        }))
    }

    type SubscribeStream = ReceiverStream<std::result::Result<ProtoMcpEvent, Status>>;
    type StreamStream = ReceiverStream<std::result::Result<ProtoMcpResponse, Status>>;
    type CallToolStreamingStream = ReceiverStream<std::result::Result<ProtoToolOutput, Status>>;

    async fn subscribe(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> std::result::Result<Response<Self::SubscribeStream>, Status> {
        Err(Status::unimplemented("Subscribe not implemented"))
    }

    async fn stream(
        &self,
        _request: Request<tonic::Streaming<ProtoMcpRequest>>,
    ) -> std::result::Result<Response<Self::StreamStream>, Status> {
        Err(Status::unimplemented("Streaming not implemented"))
    }

    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> std::result::Result<Response<HealthResponse>, Status> {
        let tool_count = self.chat_actor.tool_registry().list().await.len();
        Ok(Response::new(HealthResponse {
            healthy: true,
            version: "1.0.0".to_string(),
            server_name: "op-chat-mcp".to_string(),
            mode: 1,
            connected_agents: vec![],
            uptime_secs: 0,
        }))
    }

    async fn initialize(
        &self,
        request: Request<InitializeRequest>,
    ) -> std::result::Result<Response<InitializeResponse>, Status> {
        let req = request.into_inner();
        Ok(Response::new(InitializeResponse {
            protocol_version: "2024-11-05".to_string(),
            server_name: "op-chat".to_string(),
            server_version: "1.0.0".to_string(),
            capabilities: vec![
                "prompts".to_string(),
                "resources".to_string(),
                "tools".to_string(),
            ],
            started_agents: vec![],
            session_id: req.session_id.unwrap_or_default(),
        }))
    }

    async fn list_tools(
        &self,
        _request: Request<ListToolsRequest>,
    ) -> std::result::Result<Response<ListToolsResponse>, Status> {
        let tools = self.chat_actor.tool_registry().list().await;
        let tool_infos: Vec<_> = tools
            .iter()
            .map(|t| op_mcp::grpc::proto::ToolInfo {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: None, // ToolSchema conversion is complex; use JSON-RPC call() for full schema
                category: Some(t.category.clone()),
                tags: t.tags.clone(),
            })
            .collect();
        let total = tool_infos.len() as u32;

        Ok(Response::new(ListToolsResponse {
            tools: tool_infos,
            total,
            has_more: false,
        }))
    }

    async fn call_tool(
        &self,
        request: Request<CallToolRequest>,
    ) -> std::result::Result<Response<CallToolResponse>, Status> {
        let req = request.into_inner();

        // Convert ToolArguments to Value — extract from Generic variant or build empty
        let arguments = req
            .arguments
            .and_then(|ta| match ta.args {
                Some(op_mcp::grpc::proto::tool_arguments::Args::Generic(s)) => struct_to_value(&s),
                _ => Some(json!({})),
            })
            .unwrap_or_else(|| json!({}));

        match self
            .chat_actor
            .tool_executor()
            .execute(&req.tool_name, arguments, None)
            .await
        {
            Ok(tracked) => {
                let result_struct = tracked.result.result.as_ref().and_then(value_to_struct);

                Ok(Response::new(CallToolResponse {
                    success: tracked.success(),
                    result: result_struct,
                    error: tracked.error().cloned(),
                    duration_ms: tracked.result.duration_ms as u64,
                }))
            }
            Err(e) => Ok(Response::new(CallToolResponse {
                success: false,
                result: None,
                error: Some(format!("Execution error: {}", e)),
                duration_ms: 0,
            })),
        }
    }

    async fn call_tool_streaming(
        &self,
        _request: Request<CallToolRequest>,
    ) -> std::result::Result<Response<Self::CallToolStreamingStream>, Status> {
        Err(Status::unimplemented("Streaming tool call not implemented"))
    }

    async fn get_tool_schema(
        &self,
        request: Request<GetToolSchemaRequest>,
    ) -> std::result::Result<Response<GetToolSchemaResponse>, Status> {
        let req = request.into_inner();
        match self
            .chat_actor
            .tool_registry()
            .get_definition(&req.tool_name)
            .await
        {
            Some(_def) => Ok(Response::new(GetToolSchemaResponse {
                schema: None, // ToolSchema requires parameter-level conversion; use JSON-RPC for full schema
            })),
            None => Err(Status::not_found(format!(
                "Tool not found: {}",
                req.tool_name
            ))),
        }
    }
}

// ============================================================================
// GRPC SERVER RUNNER
// ============================================================================

pub async fn run_chat_mcp_server(addr: std::net::SocketAddr, actor: Arc<ChatActor>) -> Result<()> {
    let service = ChatMcpServer::new(actor);
    Server::builder()
        .add_service(McpServiceServer::new(service))
        .serve(addr)
        .await?;
    Ok(())
}
