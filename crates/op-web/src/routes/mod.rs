//! API routes and route handlers

use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::groups_admin;
use crate::handlers;
use crate::mcp;
use crate::mcp_agents;
use crate::mcp_discovery;
use crate::middleware::security;
use crate::sse;
use crate::state::AppState;
use crate::websocket;
use op_cognitive_mcp::context_server::build_context_router;

pub mod admin;
#[allow(dead_code)]
pub mod chat;
#[allow(dead_code)]
pub mod llm;

/// Create the complete router with all routes
pub fn create_router(state: Arc<AppState>) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // API routes
    let api_routes = Router::new()
        // Health & Status
        .route("/health", get(handlers::health::health_handler))
        .route("/status", get(handlers::status::status_handler))
        // Schema — single source of truth from shared memory
        .route("/schema", get(handlers::schema::schema_handler))
        // Compatibility paths used by dashboard builds predating `/api/schema`.
        // Keep these on the live router; `router.rs` is not mounted by main.
        .route("/schema/catalog", get(handlers::schema::schema_handler))
        .route(
            "/schema/catalog/detail",
            get(handlers::schema::schema_catalog_handler),
        )
        // Identity sled — live WireGuard identity from shared memory
        .route(
            "/identity/sled",
            get(handlers::identity::identity_sled_handler),
        )
        // Dashboard
        .route(
            "/dashboard/metrics",
            get(handlers::dashboard::dashboard_metrics_handler),
        )
        // Live SHM present-state. Replaces the projection daemon dump.
        .route(
            "/ui-model/state",
            get(handlers::ui_model::ui_model_state_handler),
        )
        .route(
            "/dashboard/projections",
            get(handlers::ui_model::ui_model_state_handler),
        )
        // Users
        .route("/users", get(handlers::users::list_users_handler))
        .route("/users/{id}", get(handlers::users::get_user_handler))
        // VPN
        .route("/vpn/status", get(handlers::vpn::vpn_status_handler))
        .route(
            "/vpn/connections",
            get(handlers::vpn::vpn_connections_handler),
        )
        .route("/vpn/config", get(handlers::vpn::vpn_config_handler))
        // Mail
        .route("/mail/status", get(handlers::mail::mail_status_handler))
        .route("/mail/queue", get(handlers::mail::mail_queue_handler))
        .route("/mail/accounts", get(handlers::mail::mail_accounts_handler))
        // Logs
        .route("/logs", get(handlers::logs::logs_handler))
        .route("/logs/stream", get(handlers::logs::logs_stream_handler))
        // Chat endpoints
        .route("/chat", post(handlers::chat::chat_handler))
        .route("/chat/stream", post(handlers::chat::chat_stream_handler))
        .route(
            "/zeroclaw/chat",
            post(handlers::zeroclaw::zeroclaw_chat_handler),
        )
        .route(
            "/zeroclaw/chat/stream",
            post(handlers::zeroclaw::zeroclaw_chat_stream_handler),
        )
        .route(
            "/zeroclaw/schema",
            get(handlers::zeroclaw::zeroclaw_schema_handler),
        )
        // Plugin identity endpoints — model-agnostic, sealed SHM blob catalog
        // (`/dev/shm/opdbus/`) is the single source of truth for both.
        .route(
            "/plugins",
            get(handlers::plugin_schema::plugin_list_handler),
        )
        .route(
            "/plugin-schema/{plugin_id}",
            get(handlers::plugin_schema::plugin_schema_handler),
        )
        // UI-model spec gallery + catalog (render slices of the sealed blob
        // PluginSchema; promote a winning lens into the catalog). Model-agnostic
        // by name: whichever model generates the spec, the surface is the same.
        .route(
            "/ui-model/gallery",
            get(handlers::ui_model::ui_model_gallery_handler),
        )
        .route(
            "/ui-model/gallery/{id}",
            delete(handlers::ui_model::ui_model_gallery_delete_handler),
        )
        .route(
            "/ui-model/catalog",
            get(handlers::ui_model::ui_model_catalog_handler),
        )
        .route(
            "/ui-model/catalog/promote/{id}",
            post(handlers::ui_model::ui_model_catalog_promote_handler),
        )
        .route(
            "/ui-model/catalog/{id}",
            delete(handlers::ui_model::ui_model_catalog_delete_handler),
        )
        .route(
            "/ui-model/plugin-schema/{plugin}",
            get(handlers::ui_model::ui_model_plugin_schema_handler),
        )
        .route(
            "/ui-model/plugins",
            get(handlers::ui_model::ui_model_list_plugins_handler),
        )
        .route(
            "/ui-model/subid-projection",
            get(handlers::ui_model::ui_model_subid_projection_handler),
        )
        // Gallery generation API (model-agnostic)
        .route(
            "/gallery-gen/start",
            post(handlers::ui_model::start_generation),
        )
        .route(
            "/gallery-gen/stop",
            post(handlers::ui_model::stop_generation),
        )
        .route(
            "/gallery-gen/stream",
            get(handlers::ui_model::generation_stream),
        )
        .route("/chat/sessions", get(handlers::chat::list_sessions_handler))
        .route(
            "/chat/sessions",
            post(handlers::chat::create_session_handler),
        )
        .route(
            "/chat/sessions/{id}",
            delete(handlers::chat::delete_session_handler),
        )
        .route("/chat/message", post(handlers::chat::send_message_handler))
        .route(
            "/chat/history/{session_id}",
            get(handlers::chat::get_history_handler),
        )
        .route(
            "/chat/transcript",
            post(handlers::chat::save_transcript_handler),
        )
        .route(
            "/chat/system-prompt",
            get(handlers::chat::get_system_prompt_handler),
        )
        .route(
            "/chat/system-prompt",
            axum::routing::put(handlers::chat::update_system_prompt_handler),
        )
        // Analytics / Accountability
        .route(
            "/analytics/semantic-search",
            get(handlers::analytics::semantic_search_handler),
        )
        // Tool endpoints
        .route("/tools", get(handlers::tools::list_tools_handler))
        .route("/tools/{name}", get(handlers::tools::get_tool_handler))
        .route("/tool", post(handlers::tools::execute_tool_handler))
        .route(
            "/tools/{name}/execute",
            post(handlers::tools::execute_named_tool_handler),
        )
        // Agent endpoints
        .route("/agents", get(handlers::agents::list_agents_handler))
        .route("/agents", post(handlers::agents::spawn_agent_handler))
        .route(
            "/agents/types",
            get(handlers::agents::list_agent_types_handler),
        )
        .route("/agents/{id}", get(handlers::agents::get_agent_handler))
        .route(
            "/agents/{id}/task",
            post(handlers::agents::agent_task_handler),
        )
        .route(
            "/agents/{id}",
            axum::routing::delete(handlers::agents::kill_agent_handler),
        )
        // LLM endpoints
        .route("/llm/status", get(handlers::llm::llm_status_handler))
        .route("/llm/providers", get(handlers::llm::list_providers_handler))
        .route("/llm/models", get(llm::get_models))
        .route(
            "/llm/models/{provider}",
            get(handlers::llm::list_models_for_provider_handler),
        )
        .route("/llm/chat", post(handlers::zeroclaw::zeroclaw_chat_handler))
        // OpenClaw endpoints (internal/base layer)
        .route(
            "/openclaw/status",
            get(handlers::openclaw::openclaw_status_handler),
        )
        .route(
            "/openclaw/config",
            get(handlers::openclaw::openclaw_config_handler),
        )
        .route(
            "/openclaw/chat",
            post(handlers::openclaw::openclaw_chat_handler),
        )
        .route(
            "/openclaw/models",
            get(handlers::openclaw::openclaw_models_handler),
        )
        // Assistant endpoints (user-facing aliases — same handlers, branded URLs)
        .route(
            "/assistant/status",
            get(handlers::openclaw::openclaw_status_handler),
        )
        .route(
            "/assistant/config",
            get(handlers::openclaw::openclaw_config_handler),
        )
        .route(
            "/assistant/chat",
            post(handlers::openclaw::openclaw_chat_handler),
        )
        .route(
            "/assistant/models",
            get(handlers::openclaw::openclaw_models_handler),
        )
        // MCP server management endpoints
        .route("/mcp/servers", get(handlers::mcp::list_servers_handler))
        .route("/mcp/servers/{id}", get(handlers::mcp::get_server_handler))
        .route(
            "/mcp/cognitive/agents",
            get(handlers::mcp::list_agents_handler),
        )
        .route(
            "/mcp/cognitive/agents",
            post(handlers::mcp::set_agents_handler),
        )
        .route(
            "/mcp/cognitive/memory",
            post(handlers::mcp::query_memory_handler),
        )
        .route(
            "/mcp/cognitive/memory/{key}",
            delete(handlers::mcp::delete_memory_handler),
        )
        .route(
            "/mcp/cognitive/memory/stats",
            get(handlers::mcp::memory_stats_handler),
        )
        // MCP discovery endpoints
        .route("/mcp/_config", get(mcp::config_handler))
        // SSE events
        .route("/events", get(sse::sse_handler))
        // Privacy router endpoints
        .route("/privacy/signup", post(handlers::privacy::signup))
        .route("/privacy/verify", get(handlers::privacy::verify))
        .route(
            "/privacy/config/{user_id}",
            get(handlers::privacy::get_config),
        )
        .route("/privacy/status", get(handlers::privacy::status))
        .route(
            "/privacy/credentials",
            post(handlers::privacy::set_credentials),
        )
        // Google OAuth endpoints
        .route("/privacy/google/auth", get(handlers::privacy::google_auth))
        .route(
            "/privacy/google/callback",
            get(handlers::privacy::google_callback),
        )
        // API misses must never fall through to the SPA and masquerade as a
        // successful HTML response.
        .fallback(api_not_found);

    // MCP JSON-RPC endpoints (profile-based and legacy)
    let mcp_route = mcp::create_mcp_router();

    // Critical Agents MCP endpoint (SSE-based, direct tool access)
    // These are added separately to avoid state conflicts
    let agents_mcp_route = Router::new()
        .route(
            "/mcp/agents",
            get(mcp_agents::mcp_agents_sse_handler_stateless),
        )
        .route(
            "/mcp/agents/message",
            post(mcp_agents::mcp_agents_message_handler_stateless),
        )
        // Phase 2 FR-2a: same MCP ingress gate as `/mcp` nest.
        .layer(axum::middleware::from_fn(
            security::mcp_ingress_auth_middleware,
        ));

    // WebSocket route
    let ws_route = Router::new().route("/ws", get(websocket::websocket_handler));

    // Main router - agents_mcp_route FIRST so it takes precedence
    let router = Router::new()
        .nest("/api", api_routes)
        // Ordinary HTTP compatibility lives on op-web :8080. Each handler is
        // an adapter to a schema-declared bridge method on gRPC :8090.
        .route("/v1/models", get(handlers::zeroclaw::openai_models_handler))
        .route(
            "/v1/chat/completions",
            post(handlers::zeroclaw::zeroclaw_chat_handler),
        )
        // Device pairing (egui / dashboard) — top-level paths match zeroclaw-gui AuthState
        .route("/pair", post(handlers::pair::pair_handler))
        // Human-facing privacy verification flow (magic-link target)
        .route("/privacy/verify", get(handlers::privacy::verify_redirect))
        .route(
            "/privacy/access",
            get(handlers::privacy::privacy_access_message),
        )
        // JSON-RPC compatibility aliases (mirror /mcp)
        .route("/jsonrpc", post(mcp::jsonrpc_handler))
        .route("/rpc", post(mcp::jsonrpc_handler))
        .merge(agents_mcp_route) // Agents first (more specific)
        // Phase 2 FR-2a: enforce zone/Ghostbridge on `/mcp` nest only (not global).
        .nest(
            "/mcp",
            mcp_route.layer(axum::middleware::from_fn(
                security::mcp_ingress_auth_middleware,
            )),
        )
        .merge(ws_route)
        // Well-known discovery endpoint for auto-configuration
        .route(
            "/.well-known/mcp.json",
            get(mcp_discovery::mcp_discovery_handler),
        )
        .nest("/groups-admin", groups_admin::create_groups_admin_router())
        .nest("/admin", admin::admin_routes());

    // Phase 2: context-awareness SSE routes (in-process, no :3003 proxy).
    // Cozo/RocksDB is single-writer — bridge owns memory.db for ToolRegistry.
    // When op-web cannot open that DB, still expose JSON health (not SPA HTML).
    let mut router = router;
    if let Some((context_engine, memory_store, session_manager)) = state.cognitive_context_state() {
        let context_router = build_context_router(context_engine, memory_store, session_manager);
        router = router.nest("/cognitive/context", context_router);
    } else {
        router = router.route(
            "/cognitive/context/health",
            get(|| async {
                (
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    serde_json::json!({
                        "status": "degraded",
                        "mode": "bridge",
                        "detail": "CozoDB locked by op-grpc-bridge; tools/context run in-process there (socket / :8090)",
                    })
                    .to_string(),
                )
            }),
        );
    }

    // Serve the pinned Lovable dashboard while keeping `/api`, `/mcp`, and
    // websocket routes on this same origin. Client-side routes fall back to
    // index.html. gRPC-Web calls (see `grpc_proxy::is_grpc_request` — routed
    // by Content-Type, not a path allowlist) are forwarded to loopback-only
    // op-grpc-bridge instead, so the dashboard's gRPC client can resolve
    // same-origin (whatever interface this request arrived on — localhost,
    // svc0/NetMaker, or the public domain) with no hardcoded upstream host
    // baked into the frontend build.
    let static_dir = std::env::var("OP_WEB_STATIC_DIR")
        .unwrap_or_else(|_| "/usr/local/share/op-dbus/dashboard".to_string());
    let spa =
        ServeDir::new(&static_dir).fallback(ServeFile::new(format!("{static_dir}/index.html")));

    let spa_or_grpc = move |req: axum::extract::Request| {
        let spa = spa.clone();
        async move {
            if crate::grpc_proxy::is_grpc_request(&req) {
                crate::grpc_proxy::proxy(req).await
            } else {
                use tower::ServiceExt;
                match spa.oneshot(req).await {
                    Ok(resp) => resp.into_response(),
                    Err(err) => {
                        tracing::error!(?err, "spa serve error");
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }
        }
    };

    router
        .fallback(spa_or_grpc)
        .layer(Extension(state))
        .layer(axum::middleware::from_fn(security::ip_security_middleware))
        .layer(axum::middleware::from_fn(
            crate::middleware::access_log::access_log_middleware,
        ))
        .layer(axum::middleware::from_fn(
            crate::middleware::spa_cache::spa_cache_middleware,
        ))
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
}

async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "API route not found",
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::api_not_found;
    use axum::{
        body::{to_bytes, Body},
        http::{header::CONTENT_TYPE, Request, StatusCode},
        response::Html,
        Router,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn missing_api_route_does_not_fall_through_to_spa() {
        let app = Router::new()
            .nest("/api", Router::new().fallback(api_not_found))
            .fallback(|| async { Html("spa index") });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/definitely-missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/json"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, r#"{"error":"API route not found"}"#);
    }
}
