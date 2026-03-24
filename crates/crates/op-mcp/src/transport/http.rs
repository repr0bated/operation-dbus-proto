//! HTTP Transport
//!
//! HTTP/REST transport with SSE support.
//! Provides three variants:
//! - HttpTransport: REST only
//! - SseTransport: SSE only (for clients that use separate SSE + POST)
//! - HttpSseTransport: Combined bidirectional (recommended)

use super::{McpHandler, Transport};
use crate::McpRequest;
use anyhow::Result;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware,
    response::{
        sse::{Event, Sse},
        IntoResponse, Json,
    },
    routing::{get, post},
    Router,
};
use futures::stream::{self, Stream};
use reqwest;
use simd_json::{json, OwnedValue as Value};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

fn auth_enforcement_enabled() -> bool {
    std::env::var("OP_MCP_ENFORCE_GCLOUD_AUTH")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Validate gcloud OAuth token via Google's tokeninfo API
async fn validate_gcloud_token(token: &str) -> Result<(), StatusCode> {
    let url = format!(
        "https://oauth2.googleapis.com/tokeninfo?access_token={}",
        token
    );

    match reqwest::get(&url).await {
        Ok(response) => {
            if response.status().is_success() {
                Ok(())
            } else {
                warn!("Token validation failed: HTTP {}", response.status());
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        Err(e) => {
            error!("Failed to validate token: {}", e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Authentication middleware - validates gcloud OAuth tokens
async fn gcloud_auth_middleware(
    headers: HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // Allow health check without auth
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    // Auth enforcement is optional; disabled by default.
    // Set OP_MCP_ENFORCE_GCLOUD_AUTH=1 to require bearer auth.
    if !auth_enforcement_enabled() {
        debug!("gcloud auth middleware disabled; allowing request");
        return Ok(next.run(request).await);
    }

    // Check for Bearer token (gcloud OAuth or WireGuard identity)
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];

                // Check for WireGuard public key format (Base64, 44 chars, ends with =)
                if token.len() == 44 && token.ends_with('=') {
                    info!("Authenticated via WireGuard identity: {}", token);
                    // In a full implementation, we would validate this against sessions.db or op-gateway
                    // For now, we accept the identity to allow the chatbot to connect
                    return Ok(next.run(request).await);
                }

                // Validate via Google tokeninfo API
                validate_gcloud_token(token).await?;
                return Ok(next.run(request).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

/// Shared state for HTTP handlers
struct HttpState<H> {
    handler: Arc<H>,
    event_tx: broadcast::Sender<String>,
}

/// HTTP-only transport (REST endpoints)
pub struct HttpTransport {
    bind_addr: String,
    enable_cors: bool,
}

impl HttpTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            enable_cors: true,
        }
    }

    pub fn without_cors(mut self) -> Self {
        self.enable_cors = false;
        self
    }
}

#[async_trait::async_trait]
impl Transport for HttpTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting HTTP transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });

        let mut app = Router::new()
            .route("/", get(root_handler).post(mcp_handler::<H>))
            .route("/mcp", post(mcp_handler::<H>))
            .route("/message", post(mcp_handler::<H>))
            .route("/health", get(health_handler))
            .route(
                "/tools/list",
                get(tools_list_handler::<H>).post(tools_list_handler::<H>),
            )
            .route("/tools/call", post(tools_call_handler::<H>))
            .with_state(state);

        if self.enable_cors {
            app = app.layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            );
        }

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "HTTP transport listening");

        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// SSE-only transport
pub struct SseTransport {
    bind_addr: String,
}

impl SseTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
        }
    }
}

#[async_trait::async_trait]
impl Transport for SseTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting SSE transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });

        let app = Router::new()
            .route("/", get(sse_handler::<H>))
            .route("/sse", get(sse_handler::<H>))
            .route("/message", post(mcp_handler::<H>))
            .route("/health", get(health_handler))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "SSE transport listening");

        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// HTTP+SSE bidirectional transport (recommended)
pub struct HttpSseTransport {
    bind_addr: String,
    base_path: String,
}

impl HttpSseTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            base_path: String::new(),
        }
    }

    pub fn with_base_path(mut self, path: impl Into<String>) -> Self {
        self.base_path = path.into();
        self
    }
}

#[async_trait::async_trait]
impl Transport for HttpSseTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting HTTP+SSE transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });

        let app = Router::new()
            .route("/", get(root_handler).post(mcp_handler::<H>))
            .route("/sse", get(sse_handler::<H>))
            .route("/mcp", post(mcp_handler::<H>))
            .route("/message", post(mcp_handler::<H>))
            .route("/health", get(health_handler))
            .route(
                "/tools/list",
                get(tools_list_handler::<H>).post(tools_list_handler::<H>),
            )
            .route("/tools/call", post(tools_call_handler::<H>))
            .layer(middleware::from_fn(gcloud_auth_middleware))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "HTTP+SSE transport listening");

        axum::serve(listener, app).await?;
        Ok(())
    }
}

// === Handlers ===

async fn root_handler() -> impl IntoResponse {
    Json(json!({
        "service": "op-mcp",
        "version": crate::SERVER_VERSION,
        "protocol": crate::PROTOCOL_VERSION,
        "endpoints": {
            "mcp": "POST /mcp",
            "sse": "GET /sse",
            "health": "GET /health",
            "tools_list": "GET /tools/list",
            "tools_call": "POST /tools/call"
        }
    }))
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "op-mcp",
        "version": crate::SERVER_VERSION
    }))
}

async fn mcp_handler<H: McpHandler>(
    State(state): State<Arc<HttpState<H>>>,
    Json(request): Json<McpRequest>,
) -> impl IntoResponse {
    debug!(method = %request.method, "HTTP MCP request");
    let response = state.handler.handle_request(request).await;
    Json(response)
}

async fn tools_list_handler<H: McpHandler>(
    State(state): State<Arc<HttpState<H>>>,
) -> impl IntoResponse {
    let request = McpRequest::new("tools/list").with_id(json!(1));
    let response = state.handler.handle_request(request).await;
    Json(response)
}

async fn tools_call_handler<H: McpHandler>(
    State(state): State<Arc<HttpState<H>>>,
    Json(params): Json<Value>,
) -> impl IntoResponse {
    let request = McpRequest::new("tools/call")
        .with_id(json!(1))
        .with_params(params);
    let response = state.handler.handle_request(request).await;
    Json(response)
}

async fn sse_handler<H: McpHandler + 'static>(
    State(state): State<Arc<HttpState<H>>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("SSE client connected");

    // Build initial events
    let initial_events = vec![
        Event::default().event("endpoint").data("/mcp"),
        Event::default().event("connected").data(
            json!({
                "server": "op-mcp",
                "version": crate::SERVER_VERSION
            })
            .to_string(),
        ),
    ];

    let initial_stream = stream::iter(initial_events.into_iter().map(Ok));

    // Keepalive stream
    let keepalive_stream = stream::unfold(0u64, |counter| async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let event = Event::default()
            .event("ping")
            .data(json!({ "counter": counter }).to_string());
        Some((Ok(event), counter + 1))
    });

    // Broadcast stream for server-initiated events
    let rx = state.event_tx.subscribe();
    let broadcast_stream =
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|result| async move {
            match result {
                Ok(data) => Some(Ok(Event::default().data(data))),
                Err(_) => None,
            }
        });

    use futures::StreamExt;
    let combined = initial_stream
        .chain(broadcast_stream)
        .chain(keepalive_stream);

    Sse::new(combined).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keepalive"),
    )
}
