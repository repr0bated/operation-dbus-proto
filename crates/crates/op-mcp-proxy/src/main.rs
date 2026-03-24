//! MCP Proxy – thin shim with optional direct-to-subscription mode.

use op_cache::proto::{mcp_service_client::McpServiceClient, McpRequest};
use simd_json::prelude::*;
use simd_json::OwnedValue;
use std::io::{BufRead, Write};
use std::sync::Arc;
use tonic::transport::Channel;
use tracing::info;

mod cloudaicompanion;
mod direct_llm;
mod gcloud_auth;
mod session;

use direct_llm::DirectLLM;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    // If DIRECT_MODE is set we handle LLM requests ourselves.
    let direct_mode = std::env::var("DIRECT_MODE").is_ok();
    let direct_llm = if direct_mode {
        info!("Running in DIRECT_MODE – LLM calls go to cloudcode-pa.googleapis.com");
        let llm = Arc::new(DirectLLM::new().await?);
        llm.start_auto_refresh();
        Some(llm)
    } else {
        None
    };

    let mut client: Option<McpServiceClient<Channel>> = if direct_mode {
        None
    } else {
        let daemon_addr =
            std::env::var("OP_DBUS_ADDR").unwrap_or_else(|_| "http://[::1]:50051".to_string());
        let channel = Channel::from_shared(daemon_addr)?.connect().await?;
        Some(McpServiceClient::new(channel))
    };

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let mut line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut line) }?;
        let method = req["method"].as_str().unwrap_or("");

        // Direct mode exposes LLM + minimal MCP protocol surface.
        if let Some(ref llm) = direct_llm {
            let direct_resp = match method {
                "completion/complete" | "sampling/createMessage" | "generate" => {
                    Some(llm.handle(&req).await)
                }
                "initialize" => Some(simd_json::json!({
                    "jsonrpc": "2.0",
                    "id": req["id"].clone(),
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
                        "serverInfo": { "name": "op-mcp-proxy", "version": "0.1.0" }
                    }
                })),
                "tools/list" => Some(simd_json::json!({
                    "jsonrpc": "2.0",
                    "id": req["id"].clone(),
                    "result": {
                        "tools": [{
                            "name": "generate",
                            "description": "Generate text using Gemini via Cloud AI Companion",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "prompt": { "type": "string", "description": "Prompt to send to model" },
                                    "model": { "type": "string", "description": "Gemini model id" }
                                },
                                "required": ["prompt"]
                            }
                        }]
                    }
                })),
                "tools/call" => Some(handle_tools_call(llm, &req).await),
                _ => Some(simd_json::json!({
                    "jsonrpc": "2.0",
                    "id": req["id"].clone(),
                    "error": { "code": -32601, "message": format!("Method not available in DIRECT_MODE: {}", method) }
                })),
            };

            if let Some(resp) = direct_resp {
                writeln!(stdout, "{}", simd_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        }

        // Otherwise forward to op-dbus daemon (original behaviour)
        let json_resp = if let Some(client) = client.as_mut() {
            let grpc_req = McpRequest {
                jsonrpc: "2.0".to_string(),
                method: req["method"].as_str().unwrap_or("").to_string(),
                id: req["id"].as_str().unwrap_or("null").to_string(),
                params: simd_json::to_vec(&req["params"]).unwrap_or_default(),
            };
            let grpc_resp = client.handle_request(grpc_req).await?.into_inner();
            if let Some(err) = grpc_resp.error {
                simd_json::json!({
                    "jsonrpc": "2.0",
                    "id": grpc_resp.id,
                    "error": { "code": err.code, "message": err.message }
                })
            } else {
                let mut result_bytes = grpc_resp.result;
                let result = simd_json::to_owned_value(&mut result_bytes)
                    .unwrap_or_else(|_| simd_json::OwnedValue::null());
                simd_json::json!({
                    "jsonrpc": "2.0",
                    "id": grpc_resp.id,
                    "result": result
                })
            }
        } else {
            simd_json::json!({
                "jsonrpc": "2.0",
                "id": req["id"].clone(),
                "error": { "code": -32601, "message": format!("Method not available in DIRECT_MODE: {}", method) }
            })
        };
        writeln!(stdout, "{}", simd_json::to_string(&json_resp)?)?;
        stdout.flush()?;
    }
    Ok(())
}

async fn handle_tools_call(llm: &Arc<DirectLLM>, req: &OwnedValue) -> OwnedValue {
    let tool_name = req["params"]["name"].as_str().unwrap_or("");
    if tool_name != "generate" {
        return simd_json::json!({
            "jsonrpc": "2.0",
            "id": req["id"].clone(),
            "error": { "code": -32601, "message": format!("Unknown tool: {}", tool_name) }
        });
    }

    let prompt = match req["params"]["arguments"]["prompt"].as_str() {
        Some(p) if !p.trim().is_empty() => p.to_string(),
        _ => {
            return simd_json::json!({
                "jsonrpc": "2.0",
                "id": req["id"].clone(),
                "error": { "code": -32602, "message": "tools/call generate requires arguments.prompt" }
            });
        }
    };

    let generated_req = simd_json::json!({
        "jsonrpc": "2.0",
        "id": req["id"].clone(),
        "method": "generate",
        "params": {
            "prompt": prompt,
            "model": req["params"]["arguments"]["model"].clone()
        }
    });

    let llm_resp = llm.handle(&generated_req).await;
    if llm_resp.get("error").is_some() {
        return llm_resp;
    }

    let text = llm_resp["result"]["completion"]
        .as_str()
        .unwrap_or("")
        .to_string();
    simd_json::json!({
        "jsonrpc": "2.0",
        "id": req["id"].clone(),
        "result": {
            "content": [{ "type": "text", "text": text }]
        }
    })
}
