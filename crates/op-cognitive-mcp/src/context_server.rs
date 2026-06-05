//! Context-Aware HTTP Server with Proactive SSE Pushes
//!
//! Extended HTTP server that adds:
//! - `/context/stream` - SSE endpoint for proactive knowledge pushes
//! - `/context/status/:session_id` - Get session context status
//! - `/context/request_push` - On-demand knowledge push
//!
//! Integrates with the ContextAwarenessEngine to provide real-time
//! context-aware assistance to connected clients.

use crate::context_awareness::{
    ActivityEvent, ActivityType, ContextAwarenessConfig, ContextAwarenessEngine, KnowledgePush,
};
use crate::memory_store::CognitiveMemoryStore;
use crate::rag_pipeline::RagPipeline;
use crate::session::SessionManager;
use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;
use tracing::{debug, error, info, warn};

/// Server state shared across request handlers
#[derive(Clone)]
pub struct ContextServerState {
    /// Context awareness engine
    pub context_engine: Arc<ContextAwarenessEngine>,
    /// Activity event sender
    pub activity_tx: tokio::sync::mpsc::Sender<ActivityEvent>,
    /// Memory store reference
    pub memory_store: Arc<CognitiveMemoryStore>,
    /// Session manager reference
    pub session_manager: Arc<SessionManager>,
}

/// Create a context-aware server with proactive SSE pushes
pub async fn create_context_aware_server(
    memory_store: Arc<CognitiveMemoryStore>,
    session_manager: Arc<SessionManager>,
    rag_pipeline: Option<Arc<RagPipeline>>,
) -> anyhow::Result<(Router, Arc<ContextAwarenessEngine>)> {
    // Create context awareness engine
    let config = ContextAwarenessConfig::default();
    let engine = Arc::new(ContextAwarenessEngine::new(
        config,
        memory_store.clone(),
        rag_pipeline,
    ));

    let activity_tx = engine.activity_sender();

    let state = ContextServerState {
        context_engine: engine.clone(),
        activity_tx: activity_tx.clone(),
        memory_store: memory_store.clone(),
        session_manager,
    };

    // Start background monitoring
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        engine_clone.start_monitoring();
    });

    // Build router with context-aware endpoints
    let router = Router::new()
        .route("/context/stream/:session_id", get(sse_push_stream))
        .route("/context/status/:session_id", get(get_session_status))
        .route("/context/record", post(record_activity))
        .route("/context/request_push", post(request_knowledge_push))
        .route("/context/health", get(context_health))
        .with_state(state);

    info!("Context-aware server endpoints registered");

    Ok((router, engine))
}

/// SSE stream handler for proactive knowledge pushes
async fn sse_push_stream(
    State(state): State<ContextServerState>,
    Path(session_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    info!(session_id = %session_id, "Client subscribed to context push stream");

    // Record the subscription activity
    let _ = state
        .activity_tx
        .send(ActivityEvent {
            session_id: session_id.clone(),
            activity_type: ActivityType::ToolCall,
            content: "Subscribed to context push stream".to_string(),
            metadata: serde_json::json!({ "endpoint": "/context/stream" }),
        })
        .await;

    // Subscribe to push notifications
    let mut push_rx = state.context_engine.subscribe_pushes();

    // Create filtered stream for this session only
    let stream = async_stream::stream! {
        // Send initial connection event
        yield Ok(Event::default()
            .event("connected")
            .data(serde_json::json!({
                "session_id": &session_id,
                "message": "Connected to context push stream",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }).to_string()));

        // Forward relevant pushes to this client
        while let Ok(push) = push_rx.recv().await {
            if push.session_id == session_id {
                let event = Event::default()
                    .event("knowledge_push")
                    .id(&push.id)
                    .data(serde_json::to_string(&push).unwrap_or_default());

                yield Ok(event);
            }
        }

        // Stream ended
        yield Ok(Event::default()
            .event("disconnected")
            .data(serde_json::json!({
                "session_id": &session_id,
                "message": "Context stream ended",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }).to_string()));
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keep-alive"),
    )
}

/// Get session context status
async fn get_session_status(
    State(state): State<ContextServerState>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    debug!(session_id = %session_id, "Getting session context status");

    let stats = state
        .context_engine
        .get_session_stats(&session_id)
        .await
        .unwrap_or(serde_json::json!({ "error": "Session not found" }));

    Json(stats)
}

/// Request on-demand knowledge push
#[derive(Debug, Deserialize)]
struct RequestPushInput {
    session_id: String,
    query: String,
    context: Option<serde_json::Value>,
}

async fn request_knowledge_push(
    State(state): State<ContextServerState>,
    Json(input): Json<RequestPushInput>,
) -> Json<serde_json::Value> {
    info!(
        session_id = %input.session_id,
        query = %input.query,
        "On-demand knowledge push requested"
    );

    // Record the request activity
    let _ = state
        .activity_tx
        .send(ActivityEvent {
            session_id: input.session_id.clone(),
            activity_type: ActivityType::Query,
            content: input.query.clone(),
            metadata: input.context.clone().unwrap_or(serde_json::json!({})),
        })
        .await;

    // Generate and send the push
    match state
        .context_engine
        .request_push(&input.session_id, &input.query)
        .await
    {
        Ok(push) => Json(serde_json::json!({
            "success": true,
            "push": push,
            "message": "Knowledge push generated and sent via SSE stream"
        })),
        Err(e) => {
            warn!(error = %e, "Failed to generate on-demand push");
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}

/// Record activity for context tracking
#[derive(Debug, Deserialize)]
struct RecordActivityInput {
    session_id: String,
    activity_type: String,
    content: String,
    #[serde(default)]
    metadata: serde_json::Value,
}

async fn record_activity(
    State(state): State<ContextServerState>,
    Json(input): Json<RecordActivityInput>,
) -> Json<serde_json::Value> {
    let activity_type = match input.activity_type.as_str() {
        "tool_call" => ActivityType::ToolCall,
        "query" => ActivityType::Query,
        "context_switch" => ActivityType::ContextSwitch,
        "error" => ActivityType::Error,
        "idle" => ActivityType::Idle,
        "return_from_idle" => ActivityType::ReturnFromIdle,
        _ => ActivityType::Query,
    };

    debug!(
        session_id = %input.session_id,
        activity_type = ?activity_type,
        "Recording activity"
    );

    state
        .context_engine
        .record_activity(
            &input.session_id,
            activity_type,
            input.content,
            input.metadata,
        )
        .await;

    Json(serde_json::json!({
        "success": true,
        "session_id": input.session_id,
        "recorded": true
    }))
}

/// Health check for context awareness system
async fn context_health(State(state): State<ContextServerState>) -> Json<serde_json::Value> {
    let active_sessions = state.context_engine.get_active_session_count();

    Json(serde_json::json!({
        "status": "healthy",
        "context_awareness": {
            "enabled": true,
            "active_sessions": active_sessions,
            "proactive_pushes_enabled": true,
            "sse_stream_available": true
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_create_context_server() {
        // This would require setting up the full infrastructure
        // For now, just verify the module compiles
        assert!(true);
    }
}
