//! OpenAI-compatible + Gemini-compatible HTTP/HTTPS gateway for Code Assist.
//!
//! Listens on two ports:
//!   HTTP  (CODE_ASSIST_HTTP_PORT,  default 8642) - plain, for tools that support custom base URL
//!   HTTPS (CODE_ASSIST_HTTPS_PORT, default 443)  - TLS, for DNS-redirected traffic from
//!                                                   generativelanguage.googleapis.com / aiplatform.googleapis.com
//!
//! Accepted endpoints:
//!   POST /v1/chat/completions                               OpenAI-compatible
//!   GET  /v1/models                                        OpenAI-compatible
//!   POST /v1beta/models/{model}:generateContent            Gemini API (generativelanguage)
//!   POST /v1/projects/{p}/locations/{l}/publishers/google/models/{model}:generateContent  Vertex AI
//!   GET  /health
//!
//! All traffic → cloudcode-pa.googleapis.com (VS Code Code Assist) → included in Enterprise Plus.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use serde::Deserialize;
use simd_json::prelude::*;
use tokio_rustls::TlsAcceptor;
use tracing::{info, warn};

use crate::direct_llm::DirectLLM;

#[derive(Clone)]
struct AppState {
    llm: Arc<DirectLLM>,
}

// ── OpenAI types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OaiRequest {
    model: Option<String>,
    messages: Vec<OaiMessage>,
    #[serde(default, rename = "stream")]
    _stream: Option<bool>,
}

#[derive(Deserialize)]
struct OaiMessage {
    role: String,
    content: Option<serde_json::Value>,
}

// ── Gemini / Vertex types ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GeminiRequest {
    contents: Option<Vec<GeminiContent>>,
    #[serde(rename = "systemInstruction")]
    system_instruction: Option<GeminiContent>,
}

#[derive(Deserialize)]
struct GeminiContent {
    role: Option<String>,
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

// ── Shared response builders ──────────────────────────────────────────────────

fn oai_response(text: String, model: &str) -> serde_json::Value {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    serde_json::json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": now,
        "model": model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": text },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 }
    })
}

fn gemini_response(text: String, model: &str) -> serde_json::Value {
    serde_json::json!({
        "candidates": [{
            "content": {
                "role": "model",
                "parts": [{ "text": text }]
            },
            "finishReason": "STOP",
            "index": 0
        }],
        "modelVersion": model
    })
}

fn err_json(msg: &str) -> serde_json::Value {
    serde_json::json!({ "error": { "message": msg, "type": "server_error" } })
}

// ── Core generate helper ──────────────────────────────────────────────────────

async fn generate(llm: &DirectLLM, prompt: &str, model: &str) -> Result<String, String> {
    let req = simd_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "generate",
        "params": { "prompt": prompt, "model": model }
    });
    let resp = llm.handle(&req).await;
    if resp.get("error").is_some() {
        let msg = resp["error"]["message"]
            .as_str()
            .unwrap_or("internal error")
            .to_string();
        return Err(msg);
    }
    Ok(resp["result"]["completion"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

// ── Prompt extractors ─────────────────────────────────────────────────────────

fn oai_to_prompt(messages: &[OaiMessage]) -> String {
    messages
        .iter()
        .filter_map(|m| {
            let text = match &m.content {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Array(parts)) => parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()).map(str::to_string))
                    .collect::<Vec<_>>()
                    .join(""),
                _ => return None,
            };
            Some(format!("{}: {}", m.role, text))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn gemini_to_prompt(req: &GeminiRequest) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(sys) = &req.system_instruction {
        if let Some(ps) = &sys.parts {
            let text: String = ps.iter().filter_map(|p| p.text.as_deref()).collect();
            if !text.is_empty() {
                parts.push(format!("system: {text}"));
            }
        }
    }
    if let Some(contents) = &req.contents {
        for c in contents {
            let role = c.role.as_deref().unwrap_or("user");
            let text: String = c
                .parts
                .as_ref()
                .map(|ps| ps.iter().filter_map(|p| p.text.as_deref()).collect())
                .unwrap_or_default();
            if !text.is_empty() {
                parts.push(format!("{role}: {text}"));
            }
        }
    }
    parts.join("\n")
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

async fn list_models() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "object": "list",
        "data": [
            { "id": "gemini-3-flash-preview",   "object": "model", "owned_by": "google-code-assist" },
            { "id": "gemini-3-pro-preview",     "object": "model", "owned_by": "google-code-assist" },
            { "id": "gemini-3-flash",           "object": "model", "owned_by": "google-code-assist" },
            { "id": "gemini-3-pro",             "object": "model", "owned_by": "google-code-assist" },
            { "id": "gemini-2.5-flash",         "object": "model", "owned_by": "google-code-assist" },
            { "id": "gemini-2.0-flash",                        "object": "model", "owned_by": "google-code-assist" },
            { "id": "gemini-3.1-pro-preview-customtools",      "object": "model", "owned_by": "google-code-assist" },
            { "id": "google/gemini-3.1-pro-preview-customtools", "object": "model", "owned_by": "google-code-assist" },
        ]
    }))
}

async fn oai_chat(State(s): State<AppState>, Json(req): Json<OaiRequest>) -> impl IntoResponse {
    let model = req
        .model
        .as_deref()
        .unwrap_or("gemini-2.5-flash")
        .to_string();
    let prompt = oai_to_prompt(&req.messages);
    if prompt.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(err_json("no content"))).into_response();
    }
    match generate(&s.llm, &prompt, &model).await {
        Ok(text) => Json(oai_response(text, &model)).into_response(),
        Err(e) => {
            warn!("generate error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err_json(&e))).into_response()
        }
    }
}

/// Handles: POST /v1beta/models/{model}:generateContent  (Gemini API format)
async fn gemini_generate(
    State(s): State<AppState>,
    Path(model): Path<String>,
    Json(req): Json<GeminiRequest>,
) -> impl IntoResponse {
    let prompt = gemini_to_prompt(&req);
    if prompt.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(err_json("no content"))).into_response();
    }
    match generate(&s.llm, &prompt, &model).await {
        Ok(text) => Json(gemini_response(text, &model)).into_response(),
        Err(e) => {
            warn!("generate error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err_json(&e))).into_response()
        }
    }
}

/// Handles: POST /v1/projects/{p}/locations/{l}/publishers/google/models/{model}:generateContent
/// (Vertex AI format - same body as Gemini API)
async fn vertex_generate(
    State(s): State<AppState>,
    Path((_project, _location, model)): Path<(String, String, String)>,
    Json(req): Json<GeminiRequest>,
) -> impl IntoResponse {
    let prompt = gemini_to_prompt(&req);
    if prompt.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(err_json("no content"))).into_response();
    }
    match generate(&s.llm, &prompt, &model).await {
        Ok(text) => Json(gemini_response(text, &model)).into_response(),
        Err(e) => {
            warn!("generate error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err_json(&e))).into_response()
        }
    }
}

// ── Router ────────────────────────────────────────────────────────────────────

fn build_router(llm: Arc<DirectLLM>) -> Router {
    let state = AppState { llm };
    Router::new()
        .route("/health", get(health))
        // OpenAI-compatible
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(oai_chat))
        // Gemini API (generativelanguage.googleapis.com format)
        .route("/v1beta/models/{model}:generateContent", post(gemini_generate))
        .route("/v1/models/{model}:generateContent", post(gemini_generate))
        // Vertex AI format
        .route(
            "/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:generateContent",
            post(vertex_generate),
        )
        .with_state(state)
}

// ── TLS cert generation ───────────────────────────────────────────────────────

fn load_or_generate_tls(cert_dir: &std::path::Path) -> anyhow::Result<TlsAcceptor> {
    use rustls::ServerConfig;
    use tokio_rustls::TlsAcceptor;

    let cert_path = cert_dir.join("cert.pem");
    let key_path = cert_dir.join("key.pem");

    let (cert_pem, key_pem) = if cert_path.exists() && key_path.exists() {
        info!("Loading existing TLS cert from {}", cert_dir.display());
        (
            std::fs::read_to_string(&cert_path)?,
            std::fs::read_to_string(&key_path)?,
        )
    } else {
        info!("Generating self-signed TLS cert in {}", cert_dir.display());
        std::fs::create_dir_all(cert_dir)?;

        let san_domains: Vec<String> = vec![
            "generativelanguage.googleapis.com".to_string(),
            "aiplatform.googleapis.com".to_string(),
            "localhost".to_string(),
        ];

        let mut params = rcgen::CertificateParams::new(san_domains)?;
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "Code Assist Local CA");

        let key_pair = rcgen::KeyPair::generate()?;
        let cert = params.self_signed(&key_pair)?;

        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();

        std::fs::write(&cert_path, &cert_pem)?;
        std::fs::write(&key_path, &key_pem)?;
        ok_cert_hint(&cert_path);

        (cert_pem, key_pem)
    };

    let certs = rustls_pemfile::certs(&mut cert_pem.as_bytes()).collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())?
        .ok_or_else(|| anyhow::anyhow!("no private key in {}", key_path.display()))?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

fn ok_cert_hint(cert_path: &std::path::Path) {
    let dir = cert_path.parent().unwrap_or(cert_path);
    info!(
        "Generated cert. Trust it on this machine:\n  \
         Linux:  sudo cp {0}/cert.pem /usr/local/share/ca-certificates/code-assist-gateway.crt && sudo update-ca-certificates\n  \
         macOS:  sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {0}/cert.pem",
        dir.display()
    );
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn run(llm: Arc<DirectLLM>) -> anyhow::Result<()> {
    // Rustls requires an explicit crypto provider when multiple crates pull in different backends.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let http_port: u16 = std::env::var("CODE_ASSIST_HTTP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8642);
    let https_port: u16 = std::env::var("CODE_ASSIST_HTTPS_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4443); // 4443 to avoid needing root; setup script does iptables REDIRECT 443→4443
    let bind = std::env::var("CODE_ASSIST_HTTP_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());

    let app = build_router(llm.clone());

    // Plain HTTP listener
    let http_addr: SocketAddr = format!("{bind}:{http_port}").parse()?;
    let http_app = app.clone();
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();
        info!("HTTP gateway on http://{http_addr}  (OpenAI-compatible)");
        axum::serve(listener, http_app).await.unwrap();
    });

    // TLS listener for DNS-redirected Gemini/Vertex traffic
    let cert_dir = std::env::var("CODE_ASSIST_CERT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/etc"))
                .join("code-assist-gateway")
        });

    match load_or_generate_tls(&cert_dir) {
        Ok(acceptor) => {
            let https_addr: SocketAddr = format!("{bind}:{https_port}").parse()?;
            info!(
                "HTTPS gateway on https://{https_addr}  (Gemini + Vertex API catch-all)\n  \
                 DNS rewrites: generativelanguage.googleapis.com → 127.0.0.1\n  \
                              aiplatform.googleapis.com → 127.0.0.1\n  \
                 iptables: 443 → {https_port}"
            );
            let listener = tokio::net::TcpListener::bind(https_addr).await?;
            loop {
                let (stream, peer) = listener.accept().await?;
                let acceptor = acceptor.clone();
                let tls_app = app.clone();
                tokio::spawn(async move {
                    match acceptor.accept(stream).await {
                        Ok(tls_stream) => {
                            let io = TokioIo::new(tls_stream);
                            let service = TowerToHyperService::new(tls_app);
                            if let Err(e) =
                                http1::Builder::new().serve_connection(io, service).await
                            {
                                tracing::debug!("TLS connection error from {peer}: {e}");
                            }
                        }
                        Err(e) => tracing::debug!("TLS handshake failed from {peer}: {e}"),
                    }
                });
            }
        }
        Err(e) => {
            warn!("TLS setup failed ({e}); HTTPS gateway disabled. HTTP only on :{http_port}");
        }
    }

    Ok(())
}
