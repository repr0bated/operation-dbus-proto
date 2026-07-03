//! op-mcp-shim — MCP (stdio) ⇄ gRPC bridge driven entirely by server reflection.
//!
//! Spawned by MCP clients (claude-code, gemini CLI, factory/droid) as a stdio
//! server. On tools/list it reflects the target gRPC endpoint and projects
//! every unary RPC as an MCP tool (schema derived from the proto descriptors);
//! tools/call builds a DynamicMessage from the JSON arguments and invokes the
//! method. New RPCs on the server appear as tools with no rebuild here.
//!
//! Identity: x-ghostbridge-* headers are normally injected in-path by xray.
//! For direct connections set GHOSTBRIDGE_FOOTPRINT / GHOSTBRIDGE_TRACE_ID or
//! pass --header.

mod codec;
mod discovery;
mod schema;

use std::str::FromStr;

use clap::Parser;
use prost_reflect::{DescriptorPool, DynamicMessage, MethodDescriptor};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tonic::codegen::http::uri::PathAndQuery;
use tonic::metadata::{Ascii, MetadataKey, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

#[derive(Parser)]
#[command(version, about = "MCP stdio shim over gRPC reflection")]
struct Cli {
    /// Target gRPC endpoint (tonic-web listener also accepts native gRPC)
    #[arg(
        long,
        env = "MCP_GRPC_ENDPOINT",
        default_value = "http://10.200.0.1:50052"
    )]
    endpoint: String,

    /// Extra request metadata, repeatable: --header k=v
    #[arg(long = "header", value_name = "K=V")]
    headers: Vec<String>,

    /// Only expose services whose full name contains this substring (repeatable)
    #[arg(long = "service")]
    services: Vec<String>,
}

struct Shim {
    channel: Channel,
    headers: Vec<(String, String)>,
    service_filter: Vec<String>,
    pool: Option<DescriptorPool>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut endpoint: Endpoint = Channel::from_shared(cli.endpoint.clone())?;
    if cli.endpoint.starts_with("https://") {
        endpoint = endpoint.tls_config(ClientTlsConfig::new().with_native_roots())?;
    }
    let channel = endpoint.connect_lazy();

    let mut headers: Vec<(String, String)> = Vec::new();
    for h in &cli.headers {
        let (k, v) = h
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("--header must be k=v, got {h:?}"))?;
        headers.push((k.to_string(), v.to_string()));
    }
    for (env, key) in [
        ("GHOSTBRIDGE_FOOTPRINT", "x-ghostbridge-footprint"),
        ("GHOSTBRIDGE_TRACE_ID", "x-ghostbridge-trace-id"),
    ] {
        if let Ok(v) = std::env::var(env) {
            if !headers.iter().any(|(k, _)| k == key) {
                headers.push((key.to_string(), v));
            }
        }
    }

    eprintln!("op-mcp-shim: target {}", cli.endpoint);

    let mut shim = Shim {
        channel,
        headers,
        service_filter: cli.services,
        pool: None,
    };
    shim.run().await
}

impl Shim {
    async fn run(&mut self) -> anyhow::Result<()> {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let req: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    respond_err(&Value::Null, -32700, &format!("parse error: {e}"));
                    continue;
                }
            };
            let id = req.get("id").cloned();
            let method = req.get("method").and_then(Value::as_str).unwrap_or("");
            let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

            if method.starts_with("notifications/") {
                continue;
            }
            let result = match method {
                "initialize" => Ok(self.initialize(&params)),
                "ping" => Ok(json!({})),
                "tools/list" => self.tools_list().await,
                "tools/call" => self.tools_call(&params).await,
                "resources/list" => Ok(json!({"resources": []})),
                "prompts/list" => Ok(json!({"prompts": []})),
                other => Err(format!("method not found: {other}")),
            };
            let Some(id) = id else { continue };
            match result {
                Ok(value) => respond_ok(&id, value),
                Err(msg) => respond_err(&id, -32603, &msg),
            }
        }
        Ok(())
    }

    fn initialize(&self, params: &Value) -> Value {
        let version = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or("2024-11-05");
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "op-mcp-shim", "version": env!("CARGO_PKG_VERSION")}
        })
    }

    async fn ensure_pool(&mut self) -> Result<&DescriptorPool, String> {
        if self.pool.is_none() {
            let pool = discovery::build_pool(self.channel.clone())
                .await
                .map_err(|e| format!("reflection discovery failed: {e:#}"))?;
            self.pool = Some(pool);
        }
        Ok(self.pool.as_ref().unwrap())
    }

    fn methods(pool: &DescriptorPool, filter: &[String]) -> Vec<(String, MethodDescriptor)> {
        let mut out = Vec::new();
        for svc in pool.services() {
            let full = svc.full_name();
            if full.starts_with("grpc.reflection") || full.starts_with("grpc.health") {
                continue;
            }
            if !filter.is_empty() && !filter.iter().any(|f| full.contains(f.as_str())) {
                continue;
            }
            for m in svc.methods() {
                if m.is_client_streaming() || m.is_server_streaming() {
                    continue;
                }
                out.push((format!("{}_{}", svc.name(), m.name()), m));
            }
        }
        out
    }

    async fn tools_list(&mut self) -> Result<Value, String> {
        let filter = self.service_filter.clone();
        let pool = self.ensure_pool().await?;
        let tools: Vec<Value> = Self::methods(pool, &filter)
            .into_iter()
            .map(|(name, m)| {
                json!({
                    "name": name,
                    "description": format!(
                        "gRPC {}.{} (request: {})",
                        m.parent_service().full_name(),
                        m.name(),
                        m.input().full_name()
                    ),
                    "inputSchema": schema::message_schema(&m.input()),
                })
            })
            .collect();
        Ok(json!({"tools": tools}))
    }

    async fn tools_call(&mut self, params: &Value) -> Result<Value, String> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or("missing tool name")?
            .to_string();
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let filter = self.service_filter.clone();
        let pool = self.ensure_pool().await?;
        let method = Self::methods(pool, &filter)
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, m)| m)
            .ok_or_else(|| format!("unknown tool: {name}"))?;

        let request_msg = DynamicMessage::deserialize(method.input(), args)
            .map_err(|e| format!("invalid arguments for {name}: {e}"))?;

        match self.unary(&method, request_msg).await {
            Ok(text) => Ok(json!({"content": [{"type": "text", "text": text}], "isError": false})),
            Err(status) => Ok(json!({
                "content": [{"type": "text", "text": format!("gRPC error {:?}: {}", status.code(), status.message())}],
                "isError": true
            })),
        }
    }

    async fn unary(
        &self,
        method: &MethodDescriptor,
        msg: DynamicMessage,
    ) -> Result<String, tonic::Status> {
        let mut grpc = tonic::client::Grpc::new(self.channel.clone());
        grpc.ready()
            .await
            .map_err(|e| tonic::Status::unavailable(format!("connect: {e}")))?;

        let path = PathAndQuery::from_str(&format!(
            "/{}/{}",
            method.parent_service().full_name(),
            method.name()
        ))
        .map_err(|e| tonic::Status::internal(format!("path: {e}")))?;

        let mut request = tonic::Request::new(msg);
        for (k, v) in &self.headers {
            let key = MetadataKey::<Ascii>::from_str(k)
                .map_err(|e| tonic::Status::internal(format!("header key {k}: {e}")))?;
            let val: MetadataValue<Ascii> = v
                .parse()
                .map_err(|_| tonic::Status::internal(format!("header value for {k}")))?;
            request.metadata_mut().insert(key, val);
        }

        let response = grpc
            .unary(request, path, codec::DynamicCodec::new(method.clone()))
            .await?;
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::pretty(&mut buf);
        serde::Serialize::serialize(response.get_ref(), &mut ser)
            .map_err(|e| tonic::Status::internal(format!("serialize response: {e}")))?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }
}

fn respond_ok(id: &Value, result: Value) {
    emit(json!({"jsonrpc": "2.0", "id": id, "result": result}));
}

fn respond_err(id: &Value, code: i64, message: &str) {
    emit(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}));
}

fn emit(v: Value) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, &v);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}
