//! HTTP Transport
//!
//! HTTP/REST transport with SSE support.
//! Provides three variants:
//! - HttpTransport: REST only
//! - SseTransport: SSE only (for clients that use separate SSE + POST)
//! - HttpSseTransport: Combined bidirectional (recommended)
//!
//! Authentication (audit item #3):
//!   - `/health` is always open.
//!   - Real socket-loopback callers bypass auth (Host header is NEVER trusted).
//!   - All other callers must present `Authorization: Bearer <token>` AND the
//!     token must be accepted by the configured [`AuthValidator`].
//!   - The default validator is fail-secure: it only accepts tokens listed in
//!     `OPDBUS_MCP_ALLOWED_PEERS`. If that env var is unset/empty, every
//!     bearer token is rejected.

use super::{McpHandler, Transport};
use crate::McpRequest;
use anyhow::Result;
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{
        sse::{Event, Sse},
        IntoResponse, Json, Response,
    },
    routing::{get, post},
    Router,
};
use futures::stream::{self, Stream};
use simd_json::{json, OwnedValue as Value};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, warn};
use uuid::Uuid;

// =============================================================================
// AuthValidator (audit item #3)
//
// The previous implementation accepted ANY token that matched the *shape* of a
// WireGuard pubkey or a UUID. That was not authentication; it was a regex.
// Real authorization now goes through an `AuthValidator`. The default
// `EnvAllowListValidator` is fail-secure: with no entries, it rejects every
// bearer token, and only loopback callers can reach the handlers.
// =============================================================================

/// Validates an opaque bearer token against an authoritative source
/// (WireGuard peer DB, A.N.N.A. Scribe session ledger, etc.).
#[async_trait::async_trait]
pub trait AuthValidator: Send + Sync + 'static {
    async fn validate(&self, token: &str) -> bool;
}

/// Default validator backed by the `OPDBUS_MCP_ALLOWED_PEERS` env var
/// (comma-separated list of allowed pubkeys and/or session UUIDs).
///
/// * If the env var is unset or empty, **every** bearer token is rejected.
/// * Comparisons are constant-time over the candidate's length to prevent
///   timing oracles on partial matches.
pub struct EnvAllowListValidator {
    allowed: Vec<String>,
}

impl EnvAllowListValidator {
    /// Read the allow-list from `OPDBUS_MCP_ALLOWED_PEERS`.
    pub fn from_env() -> Self {
        let raw = std::env::var("OPDBUS_MCP_ALLOWED_PEERS").unwrap_or_default();
        let allowed: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if allowed.is_empty() {
            warn!(
                "OPDBUS_MCP_ALLOWED_PEERS is empty; bearer-token auth will reject all tokens. \
                 Only loopback callers will be accepted."
            );
        } else {
            info!(count = allowed.len(), "Loaded MCP peer allow-list");
        }
        Self { allowed }
    }

    /// Construct an allow-list directly (primarily for tests and embedders).
    pub fn new<I, S>(entries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            allowed: entries.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait::async_trait]
impl AuthValidator for EnvAllowListValidator {
    async fn validate(&self, token: &str) -> bool {
        // Cheap defense-in-depth pre-filter: reject anything that cannot
        // possibly be a WireGuard pubkey or session UUID before we even
        // touch the allow-list.
        if !is_wireguard_auth_token_shape(token) {
            return false;
        }
        let token_bytes = token.as_bytes();
        // OR the comparison results so total work is independent of which
        // (if any) entry matches. `ct_eq` itself runs in time proportional
        // only to the shared length of its inputs.
        let mut matched = false;
        for entry in &self.allowed {
            matched |= ct_eq(token_bytes, entry.as_bytes());
        }
        matched
    }
}

/// Constant-time byte-slice equality. Returns false on length mismatch
/// (length is not considered secret; the contents are).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn is_wireguard_pubkey_shape(token: &str) -> bool {
    token.len() == 44
        && token.ends_with('=')
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}

fn is_wireguard_session_id_shape(token: &str) -> bool {
    Uuid::parse_str(token).is_ok()
}

/// **Shape check only.** Used as a cheap pre-filter inside the validator.
/// MUST NOT be used as an authorization decision on its own.
fn is_wireguard_auth_token_shape(token: &str) -> bool {
    is_wireguard_pubkey_shape(token) || is_wireguard_session_id_shape(token)
}

fn is_loopback_addr(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

/// Authentication middleware.
///
/// Loopback bypass uses the **actual socket peer address**, not the
/// attacker-controlled `Host` header. Any non-loopback caller must present a
/// bearer token accepted by the configured [`AuthValidator`].
async fn wireguard_auth_middleware(
    State(validator): State<Arc<dyn AuthValidator>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // /health is always open
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    // Loopback bypass uses the real peer IP. The Host header is never trusted.
    if let Some(ConnectInfo(peer)) = connect_info {
        if is_loopback_addr(&peer) {
            return Ok(next.run(request).await);
        }
    }

    let Some(token) = extract_bearer_token(&headers) else {
        warn!("Rejected HTTP MCP request without bearer token");
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !validator.validate(token).await {
        warn!("Rejected HTTP MCP request: bearer token not in allow-list");
        return Err(StatusCode::UNAUTHORIZED);
    }

    debug!("Accepted HTTP MCP request with validated bearer token");
    request.extensions_mut().insert(token.to_string());
    Ok(next.run(request).await)
}

/// Shared state for HTTP handlers
struct HttpState<H> {
    handler: Arc<H>,
    event_tx: broadcast::Sender<String>,
}

fn default_validator() -> Arc<dyn AuthValidator> {
    Arc::new(EnvAllowListValidator::from_env())
}

/// HTTP-only transport (REST endpoints)
pub struct HttpTransport {
    bind_addr: String,
    enable_cors: bool,
    validator: Arc<dyn AuthValidator>,
}

impl HttpTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            enable_cors: true,
            validator: default_validator(),
        }
    }

    pub fn without_cors(mut self) -> Self {
        self.enable_cors = false;
        self
    }

    /// Inject a custom authorization backend (e.g. a live WireGuard peer DB).
    /// Defaults to [`EnvAllowListValidator::from_env`].
    pub fn with_auth_validator(mut self, validator: Arc<dyn AuthValidator>) -> Self {
        self.validator = validator;
        self
    }
}

#[async_trait::async_trait]
impl Transport for HttpTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting HTTP transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });
        let validator = self.validator;

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
            .layer(middleware::from_fn_with_state(
                validator,
                wireguard_auth_middleware,
            ))
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

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
        Ok(())
    }
}

/// SSE-only transport
pub struct SseTransport {
    bind_addr: String,
    validator: Arc<dyn AuthValidator>,
}

impl SseTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            validator: default_validator(),
        }
    }

    pub fn with_auth_validator(mut self, validator: Arc<dyn AuthValidator>) -> Self {
        self.validator = validator;
        self
    }
}

#[async_trait::async_trait]
impl Transport for SseTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting SSE transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });
        let validator = self.validator;

        let app = Router::new()
            .route("/", get(sse_handler::<H>))
            .route("/sse", get(sse_handler::<H>))
            .route("/message", post(mcp_handler::<H>))
            .route("/health", get(health_handler))
            .layer(middleware::from_fn_with_state(
                validator,
                wireguard_auth_middleware,
            ))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "SSE transport listening");

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
        Ok(())
    }
}

/// HTTP+SSE bidirectional transport (recommended)
pub struct HttpSseTransport {
    bind_addr: String,
    base_path: String,
    validator: Arc<dyn AuthValidator>,
}

impl HttpSseTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            base_path: String::new(),
            validator: default_validator(),
        }
    }

    pub fn with_base_path(mut self, path: impl Into<String>) -> Self {
        self.base_path = path.into();
        self
    }

    pub fn with_auth_validator(mut self, validator: Arc<dyn AuthValidator>) -> Self {
        self.validator = validator;
        self
    }
}

#[async_trait::async_trait]
impl Transport for HttpSseTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting HTTP+SSE transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });
        let base_path = self.base_path.trim_end_matches('/').to_string();
        let validator = self.validator;

        let mut app = Router::new()
            .route("/", get(root_handler).post(mcp_handler::<H>))
            .route("/sse", get(sse_handler::<H>))
            .route("/mcp", post(mcp_handler::<H>))
            .route("/message", post(mcp_handler::<H>))
            .route("/health", get(health_handler))
            .route(
                "/tools/list",
                get(tools_list_handler::<H>).post(tools_list_handler::<H>),
            )
            .route("/tools/call", post(tools_call_handler::<H>));

        if !base_path.is_empty() {
            app = app
                .route(&base_path, get(sse_handler::<H>).post(mcp_handler::<H>))
                .route(&format!("{}/sse", base_path), get(sse_handler::<H>))
                .route(&format!("{}/message", base_path), post(mcp_handler::<H>))
                .route(
                    &format!("{}/tools/list", base_path),
                    get(tools_list_handler::<H>).post(tools_list_handler::<H>),
                )
                .route(
                    &format!("{}/tools/call", base_path),
                    post(tools_call_handler::<H>),
                );
        }

        let app = app
            .layer(middleware::from_fn_with_state(
                validator,
                wireguard_auth_middleware,
            ))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "HTTP+SSE transport listening");

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
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
) -> Response {
    debug!(method = %request.method, "HTTP MCP request");
    let is_notification = request.id.is_none();
    let response = state.handler.handle_request(request).await;

    if is_notification {
        StatusCode::ACCEPTED.into_response()
    } else {
        Json(response).into_response()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn should_extract_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer 123e4567-e89b-12d3-a456-426614174000"),
        );

        assert_eq!(
            extract_bearer_token(&headers),
            Some("123e4567-e89b-12d3-a456-426614174000")
        );
    }

    #[test]
    fn shape_check_accepts_pubkey_and_uuid() {
        assert!(is_wireguard_auth_token_shape(
            "123e4567-e89b-12d3-a456-426614174000"
        ));
        assert!(is_wireguard_auth_token_shape(
            "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY="
        ));
        assert!(!is_wireguard_auth_token_shape("ya29.google-oauth-token"));
        assert!(!is_wireguard_auth_token_shape("not-a-wireguard-token"));
    }

    #[test]
    fn ct_eq_matches_only_identical_inputs() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
        assert!(!ct_eq(b"", b"a"));
        assert!(ct_eq(b"", b""));
    }

    #[tokio::test]
    async fn validator_accepts_listed_token_and_rejects_others() {
        let v = EnvAllowListValidator::new(vec!["123e4567-e89b-12d3-a456-426614174000"]);
        assert!(v.validate("123e4567-e89b-12d3-a456-426614174000").await);
        assert!(!v.validate("00000000-0000-0000-0000-000000000000").await);
        assert!(!v.validate("garbage").await);
        // wrong shape is rejected even though it would be exactly equal to no
        // entry anyway
        assert!(!v.validate("not-a-wireguard-token").await);
    }

    #[tokio::test]
    async fn validator_with_no_entries_rejects_everything() {
        let v = EnvAllowListValidator::new(Vec::<String>::new());
        assert!(!v.validate("123e4567-e89b-12d3-a456-426614174000").await);
        assert!(
            !v.validate("MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=")
                .await
        );
    }
}
