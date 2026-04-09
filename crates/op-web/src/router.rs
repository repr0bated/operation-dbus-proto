//! Router configuration for op-web

use axum::{
    http::Uri,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::embedded_ui::serve_embedded_ui;
use crate::routes;
use crate::state::AppState;
use crate::websocket::handle_websocket;

/// Web service router builder
pub struct WebServiceRouter;

impl WebServiceRouter {
    /// Create a new router with all routes
    pub fn new() -> Self {
        Self
    }
}

/// Embedded UI fallback handler - serves SPA for all non-API routes
async fn ui_fallback(uri: Uri) -> impl axum::response::IntoResponse {
    serve_embedded_ui(uri).await
}

/// Create the main application router
pub fn create_router(state: Arc<AppState>, _static_dir: Option<String>) -> Router {
    let router = Router::new()
        // Health check
        .route("/api/health", get(routes::health_check))
        // LLM routes
        .route("/api/llm/status", get(routes::llm::get_llm_status))
        .route("/api/llm/models", get(routes::llm::get_models))
        .route("/api/llm/provider", post(routes::llm::switch_provider))
        .route("/api/llm/model", post(routes::llm::switch_model))
        // Chat routes
        .route("/api/chat", post(routes::chat::chat_message))
        // Tool routes
        .route("/api/tools", get(routes::tools::list_tools))
        .route("/api/tools/:name", get(routes::tools::get_tool))
        .route(
            "/api/tools/:name/execute",
            post(routes::tools::execute_tool),
        )
        // WebSocket
        .route("/ws", get(handle_websocket))
        .with_state(state);

    router.fallback(ui_fallback)
}

/// Create WebSocket-only router (for composition)
pub fn create_websocket_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/ws", get(handle_websocket))
        .with_state(state)
}
