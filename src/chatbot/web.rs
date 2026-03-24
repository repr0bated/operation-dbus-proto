//! Web interface for chatbot.
//!
//! Provides HTTP and WebSocket endpoints for:
//! - Chat interaction
//! - Live MCP event streaming
//! - Session management

use std::sync::Arc;
use axum::{
    Router,
    routing::{get, post},
    extract::{State, Path, WebSocketUpgrade, ws::{WebSocket, Message}},
    response::{Html, IntoResponse, Response},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use tokio::sync::broadcast;
use futures::{SinkExt, StreamExt};

use super::{
    Chatbot, ChatbotResponse, UserMessage,
    session::SessionId,
    live_mcp::LiveMcpEvent,
    tools::UserResponseEvent,
};

/// Web server state
#[derive(Clone)]
pub struct WebState {
    pub chatbot: Arc<Chatbot>,
    pub response_rx: broadcast::Sender<UserResponseEvent>,
}

/// Chat request body
#[derive(Deserialize)]
pub struct ChatRequest {
    pub session_id: Option<String>,
    pub message: String,
}

/// Chat response body
#[derive(Serialize)]
pub struct ChatResponseBody {
    pub session_id: String,
    pub response: ChatbotResponse,
}

/// Session info response
#[derive(Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub created_at_ns: u128,
    pub exchange_count: usize,
    pub tools_used: Vec<String>,
}

/// Build the web router
pub fn build_router(state: WebState) -> Router {
    Router::new()
        // Chat endpoints
        .route("/", get(index_handler))
        .route("/api/chat", post(chat_handler))
        .route("/api/sessions", get(list_sessions_handler))
        .route("/api/sessions/:session_id", get(get_session_handler))
        .route("/api/sessions/:session_id", axum::routing::delete(delete_session_handler))
        .route("/api/tools", get(list_tools_handler))
        // WebSocket endpoints
        .route("/ws/chat/:session_id", get(ws_chat_handler))
        .route("/ws/live", get(ws_live_handler))
        // Health check
        .route("/health", get(health_handler))
        .with_state(state)
}

/// Index page with embedded chat UI
async fn index_handler() -> Html<&'static str> {
    Html(include_str!("web_ui.html"))
}

/// Handle chat request
async fn chat_handler(
    State(state): State<WebState>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponseBody>, (StatusCode, String)> {
    let session_id = request.session_id
        .map(SessionId::from_string)
        .unwrap_or_else(SessionId::new);

    let user_message = UserMessage {
        session_id: session_id.clone(),
        content: request.message,
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    };

    match state.chatbot.process(user_message).await {
        Ok(response) => Ok(Json(ChatResponseBody {
            session_id: session_id.to_string(),
            response,
        })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// List all sessions
async fn list_sessions_handler(
    State(state): State<WebState>,
) -> Json<Vec<SessionInfo>> {
    let sessions = state.chatbot.sessions();
    let infos: Vec<_> = sessions.list_sessions()
        .into_iter()
        .filter_map(|id| {
            sessions.get(&id).map(|s| SessionInfo {
                session_id: id.to_string(),
                created_at_ns: s.created_at_ns,
                exchange_count: s.exchanges.len(),
                tools_used: s.executed_tools.clone(),
            })
        })
        .collect();
    Json(infos)
}

/// Get session details
async fn get_session_handler(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let id = SessionId::from_string(session_id);
    match state.chatbot.sessions().get(&id) {
        Some(session) => Ok(Json(simd_json::json!({
            "session_id": session.id.to_string(),
            "created_at_ns": session.created_at_ns,
            "last_activity_ns": session.last_activity_ns,
            "exchanges": session.exchanges,
            "executed_tools": session.executed_tools,
            "entities": session.entities,
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Delete a session
async fn delete_session_handler(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
) -> StatusCode {
    let id = SessionId::from_string(session_id);
    match state.chatbot.sessions().remove(&id) {
        Some(_) => StatusCode::NO_CONTENT,
        None => StatusCode::NOT_FOUND,
    }
}

/// List available tools
async fn list_tools_handler(
    State(state): State<WebState>,
) -> Result<Json<Vec<String>>, StatusCode> {
    match state.chatbot.discover_tools().await {
        Ok(tools) => Ok(Json(tools)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Health check
async fn health_handler() -> Json<Value> {
    Json(simd_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// WebSocket chat handler
async fn ws_chat_handler(
    ws: WebSocketUpgrade,
    State(state): State<WebState>,
    Path(session_id): Path<String>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws_chat(socket, state, session_id))
}

async fn handle_ws_chat(
    socket: WebSocket,
    state: WebState,
    session_id: String,
) {
    let (mut sender, mut receiver) = socket.split();
    let session_id = SessionId::from_string(session_id);

    // Subscribe to responses for this session
    let mut response_rx = state.response_rx.subscribe();
    let filter_session = session_id.to_string();

    // Task to forward responses to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(event) = response_rx.recv().await {
            if event.session_id == filter_session {
                let msg = simd_json::json!({
                    "type": "response",
                    "message": event.message,
                    "format": event.format,
                    "execution_hash": event.execution_hash,
                });
                if sender.send(Message::Text(msg.to_string())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages
    let chatbot = state.chatbot.clone();
    let recv_session_id = session_id.clone();
    
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            let user_message = UserMessage {
                session_id: recv_session_id.clone(),
                content: text,
                timestamp_ns: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            };

            // Process message (response will come through channel)
            let _ = chatbot.process(user_message).await;
        }
    }

    send_task.abort();
}

/// WebSocket live MCP event handler
async fn ws_live_handler(
    ws: WebSocketUpgrade,
    State(state): State<WebState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws_live(socket, state))
}

async fn handle_ws_live(
    socket: WebSocket,
    state: WebState,
) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to live MCP events if enabled
    if let Some(live_mcp) = state.chatbot.live_mcp() {
        let mut event_rx = live_mcp.subscribe();

        let send_task = tokio::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                let msg = simd_json::to_string(&event).unwrap_or_default();
                if sender.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        });

        // Keep connection alive until client disconnects
        while let Some(Ok(_)) = receiver.next().await {}

        send_task.abort();
    }
}

/// Start the web server
pub async fn start_web_server(
    chatbot: Arc<Chatbot>,
    response_channel: broadcast::Sender<UserResponseEvent>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = WebState {
        chatbot,
        response_rx: response_channel,
    };

    let app = build_router(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Chatbot web interface listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
