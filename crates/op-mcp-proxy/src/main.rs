//! [DEPRECATED] MCP Proxy – thin shim. REMOVE ME.
//!
//! This crate is deprecated. zeroclaw already owns the equivalent functionality.
//!
//! Correct identity foundation (do not contradict this in code/docs):
//! - WireGuard key-backed identity is the root.
//! - wg connect → xray validates wg keypair + injects trusted source headers
//!   (Ghostbridge headers + NextDNS info).
//! - Recorded in **identity sled** + **Snowball ledger** (persistent for account lifetime).
//! - No SQL for users/sessions. Legacy SQL catalogs are obsolete.
//!
//! compact-mcp (loopback only) and any op-mcp-proxy style shims are not for external routing.
//!
//! Routing below is historical/legacy only.

use op_cache::proto::{mcp_service_client::McpServiceClient, McpRequest};
use simd_json::prelude::*;
use simd_json::OwnedValue;
use std::io::{BufRead, Write};
use std::sync::Arc;
use tonic::transport::Channel;
use tracing::{info, warn};

mod cloudaicompanion;
mod codex;
mod direct_llm;
mod gcloud_auth;
mod http_server;
mod session;
mod sled;
mod vertex_grpc;

use direct_llm::DirectLLM;
use sled::SledSnapshot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    // Read identity sled — footprint + trace-id for Ghostbridge header injection.
    let snapshot = SledSnapshot::read();
    if let Some(ref s) = snapshot {
        if s.is_valid {
            info!(
                footprint = %s.footprint_hex,
                trace_id  = %s.trace_id,
                nextdns   = %s.nextdns_profile,
                "Identity sled loaded"
            );
        } else {
            warn!("Identity sled present but is_valid=false — headers will be omitted");
        }
    } else {
        warn!(
            "Identity sled not found at {} — Ghostbridge headers disabled",
            sled::SLED_PATH
        );
    }

    // Xray SOCKS5 proxy — only used when XRAY_SOCKS_ADDR is explicitly set to a non-empty value.
    let xray_socks_env = std::env::var("XRAY_SOCKS_ADDR").unwrap_or_default();
    let xray_socks = xray_socks_env.as_str();
    let use_xray = !xray_socks.is_empty() && snapshot.as_ref().map(|s| s.is_valid).unwrap_or(false);

    // Initialize ChatManager to discover active providers/models.
    let chat_manager = Arc::new(op_llm::chat::ChatManager::new());

    // If DIRECT_MODE is set we handle LLM requests ourselves.
    let direct_mode = std::env::var("DIRECT_MODE").is_ok();
    let direct_llm = if direct_mode {
        info!(
            via_xray = use_xray,
            "Running in DIRECT_MODE – LLM calls go to cloudcode-pa.googleapis.com"
        );
        let llm = Arc::new(
            DirectLLM::new_with_proxy(if use_xray { Some(xray_socks) } else { None }).await?,
        );
        llm.start_auto_refresh();

        // Spawn OpenAI-compatible HTTP server in background only when not in HTTP_ONLY mode
        // (HTTP_ONLY runs the server in the main thread instead).
        if let Ok(http_addr) = std::env::var("HTTP_SERVER_ADDR") {
            if std::env::var("HTTP_ONLY").is_err() {
                let llm_clone = Arc::clone(&llm);
                let cm_clone = Arc::clone(&chat_manager);
                tokio::spawn(async move {
                    if let Err(e) = http_server::run(Some(llm_clone), cm_clone, &http_addr).await {
                        tracing::error!("HTTP server error: {}", e);
                    }
                });
            }
        }

        Some(llm)
    } else {
        None
    };

    // gRPC client for op-dbus — always connect; DIRECT_MODE only changes LLM routing.
    let daemon_addr =
        std::env::var("OP_DBUS_ADDR").unwrap_or_else(|_| "http://10.200.0.2:50051".to_string());
    info!(addr = %daemon_addr, direct_mode, "Connecting to op-dbus gRPC");
    let mut client: Option<McpServiceClient<Channel>> =
        match Channel::from_shared(daemon_addr.clone()) {
            Ok(builder) => Some(McpServiceClient::new(builder.connect_lazy())),
            Err(e) => {
                warn!("Invalid op-dbus address {}: {}", daemon_addr, e);
                None
            }
        };

    // HTTP-only mode: spawn the HTTP server (Vertex AI or CloudAI) and wait for signal.
    if std::env::var("HTTP_ONLY").is_ok() {
        if let Ok(http_addr) = std::env::var("HTTP_SERVER_ADDR") {
            if let Err(e) = http_server::run(
                direct_llm.map(|l| Arc::clone(&l)),
                Arc::clone(&chat_manager),
                &http_addr,
            )
            .await
            {
                tracing::error!("HTTP server error: {}", e);
            }
        } else {
            tokio::signal::ctrl_c().await?;
        }
        return Ok(());
    }

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let mut line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut line) }?;
        let method = req["method"].as_str().unwrap_or("");

        // Direct mode: intercept Gemini LLM methods only; everything else falls through to op-dbus.
        if let Some(ref llm) = direct_llm {
            let is_gemini = req
                .get("params")
                .and_then(|p| p.get("model"))
                .and_then(|m| m.as_str())
                .map(|m| m.to_ascii_lowercase().starts_with("gemini"))
                .unwrap_or(false);

            let direct_resp = match method {
                "completion/complete" | "sampling/createMessage" | "generate" if is_gemini => {
                    Some(llm.handle(&req).await)
                }
                "tools/call" => {
                    let tool_name = req
                        .get("params")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    if tool_name == "generate" {
                        Some(handle_tools_call(llm, &req).await)
                    } else {
                        None // forward op-dbus tools to gRPC
                    }
                }
                _ => None, // forward everything else (initialize, tools/list, op-dbus calls) to op-dbus
            };

            if let Some(resp) = direct_resp {
                writeln!(stdout, "{}", simd_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        }

        // Forward to op-dbus via gRPC with Ghostbridge identity headers.
        let json_resp = if let Some(client) = client.as_mut() {
            let grpc_req = McpRequest {
                jsonrpc: "2.0".to_string(),
                method: req["method"].as_str().unwrap_or("").to_string(),
                id: req["id"].as_str().unwrap_or("null").to_string(),
                params: simd_json::to_vec(&req["params"]).unwrap_or_default(),
            };

            // Wrap in tonic::Request and inject Ghostbridge headers from sled.
            let mut tonic_req = tonic::Request::new(grpc_req);
            if let Some(ref s) = snapshot {
                if s.is_valid {
                    if let (Ok(fp), Ok(tr)) = (
                        s.footprint_hex.parse::<tonic::metadata::MetadataValue<_>>(),
                        s.trace_id.parse::<tonic::metadata::MetadataValue<_>>(),
                    ) {
                        tonic_req
                            .metadata_mut()
                            .insert("x-ghostbridge-footprint", fp);
                        tonic_req
                            .metadata_mut()
                            .insert("x-ghostbridge-trace-id", tr);
                    }
                }
            }

            match client.handle_request(tonic_req).await {
                Ok(resp) => {
                    let grpc_resp = resp.into_inner();
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
                }
                Err(e) => simd_json::json!({
                    "jsonrpc": "2.0",
                    "id": req["id"].clone(),
                    "error": { "code": -32603, "message": format!("gRPC error: {}", e) }
                }),
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
