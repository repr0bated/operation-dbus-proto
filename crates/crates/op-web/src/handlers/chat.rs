//! Chat API Handlers

use axum::{
    extract::{Extension, Path},
    response::{
        sse::{Event, Sse},
        Json,
    },
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::orchestrator::OrchestratorEvent;
use crate::state::AppState;
use op_llm::provider::ChatMessage;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>, // For user-specific API credentials
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub success: bool,
    pub message: String,
    pub error: Option<String>,
    pub tools_executed: Vec<String>,
    pub session_id: String,
    pub model: String,
    pub provider: String,
}

/// POST /api/chat - Main chat endpoint (Blocking)
pub async fn chat_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Json<ChatResponse> {
    info!(
        "Chat request: {} chars, user: {:?}",
        request.message.len(),
        request.user_id
    );

    let session_id = request
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    {
        let mut conversations = state.conversations.write().await;
        let history = conversations.entry(session_id.clone()).or_default();
        history.push(ChatMessage::user(request.message.clone()));
    }

    match state
        .orchestrator
        .process(&session_id, &request.message, None)
        .await
    {
        Ok(result) => {
            let provider = state.chat_manager.current_provider().await.to_string();
            let model = state.chat_manager.current_model().await;
            let response_message = result.message.clone();

            if result.success {
                let mut conversations = state.conversations.write().await;
                if let Some(history) = conversations.get_mut(&session_id) {
                    history.push(ChatMessage::assistant(response_message.clone()));
                }
            }

            Json(ChatResponse {
                success: result.success,
                message: if result.success {
                    response_message.clone()
                } else {
                    String::new()
                },
                error: if result.success {
                    None
                } else {
                    Some(response_message)
                },
                tools_executed: result.tools_executed,
                session_id,
                model,
                provider,
            })
        }
        Err(e) => {
            let provider = state.chat_manager.current_provider().await.to_string();
            let model = state.chat_manager.current_model().await;
            Json(ChatResponse {
                success: false,
                message: String::new(),
                error: Some(e.to_string()),
                tools_executed: vec![],
                session_id,
                model,
                provider,
            })
        }
    }
}

/// POST /api/chat/stream - Streaming chat endpoint (SSE)
pub async fn chat_stream_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let session_id = request
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let (tx, mut rx) = mpsc::channel(100);
    let state_clone = state.clone();
    let message = request.message.clone();
    let session_clone = session_id.clone();

    {
        let mut conversations = state.conversations.write().await;
        let history = conversations.entry(session_id.clone()).or_default();
        history.push(ChatMessage::user(request.message.clone()));
    }

    tokio::spawn(async move {
        let result = state_clone
            .orchestrator
            .process(&session_clone, &message, Some(tx.clone()))
            .await;

        match result {
            Ok(response) => {
                let _ = tx
                    .send(OrchestratorEvent::Finished {
                        success: response.success,
                        message: response.message.clone(),
                        tools_executed: response.tools_executed.clone(),
                    })
                    .await;
            }
            Err(e) => {
                let _ = tx
                    .send(OrchestratorEvent::Error {
                        message: e.to_string(),
                    })
                    .await;
            }
        }
    });

    // Create stream from receiver
    let stream = async_stream::stream! {
        while let Some(event) = rx.recv().await {
            yield Ok(Event::default().data(simd_json::to_string(&event).unwrap_or_default()));
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

/// GET /api/chat/history/:session_id - Get conversation history
pub async fn get_history_handler(
    Extension(state): Extension<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<Value> {
    let conversations = state.conversations.read().await;

    if let Some(history) = conversations.get(&session_id) {
        Json(json!({
            "session_id": session_id,
            "messages": history.iter().map(|m| json!({
                "role": m.role,
                "content": m.content
            })).collect::<Vec<_>>()
        }))
    } else {
        Json(json!({
            "session_id": session_id,
            "messages": []
        }))
    }
}

/// GET /api/chat/sessions - List all chat sessions
pub async fn list_sessions_handler(Extension(state): Extension<Arc<AppState>>) -> Json<Value> {
    let conversations = state.conversations.read().await;

    let sessions: Vec<Value> = conversations
        .iter()
        .map(|(session_id, history)| {
            let title = if let Some(first_msg) = history.first() {
                first_msg.content.chars().take(50).collect::<String>()
            } else {
                "Empty session".to_string()
            };

            json!({
                "id": session_id,
                "title": title,
                "date": chrono::Utc::now().to_rfc3339(),
                "messages": history.len()
            })
        })
        .collect();

    Json(json!(sessions))
}

/// POST /api/chat/sessions - Create a new chat session
pub async fn create_session_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<CreateSessionRequest>,
) -> Json<Value> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let title = payload.title.unwrap_or_else(|| "New Chat".to_string());

    // Initialize empty conversation history
    let mut conversations = state.conversations.write().await;
    conversations.insert(session_id.clone(), vec![]);
    drop(conversations);

    Json(json!({
        "id": session_id,
        "title": title,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "updated_at": chrono::Utc::now().to_rfc3339(),
        "message_count": 0
    }))
}

/// DELETE /api/chat/sessions/:id - Delete a chat session
pub async fn delete_session_handler(
    Extension(state): Extension<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<Value> {
    let mut conversations = state.conversations.write().await;
    conversations.remove(&session_id);

    Json(json!({
        "success": true,
        "message": format!("Session {} deleted", session_id)
    }))
}

/// POST /api/chat/message - Send a message (creates new session if no session_id provided)
pub async fn send_message_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(mut request): Json<ChatRequest>,
) -> Json<ChatResponse> {
    // If no session_id, create a new one
    if request.session_id.is_none() {
        request.session_id = Some(uuid::Uuid::new_v4().to_string());
    }

    chat_handler(Extension(state), Json(request)).await
}

/// GET /api/chat/system-prompt - Get system prompt
pub async fn get_system_prompt_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Value> {
    let fixed = op_chat::system_prompt::get_fixed_prompt();
    let (custom, _source) = op_chat::system_prompt::load_custom_prompt().await;

    json!({
        "immutable": fixed,
        "tunable": custom,
        "custom": custom  // Alias for compatibility
    })
    .into()
}

/// PUT /api/chat/system-prompt - Update custom system prompt
pub async fn update_system_prompt_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let content = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");

    match op_chat::system_prompt::save_custom_prompt(content).await {
        Ok(path) => json!({
            "success": true,
            "message": "System prompt updated successfully",
            "path": path
        }),
        Err(e) => json!({
            "success": false,
            "error": e.to_string()
        }),
    }
    .into()
}

/// POST /api/chat/transcript - Save conversation transcript to file
/// Accepts either a session_id to save from memory, or direct messages array
pub async fn save_transcript_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<Value>,
) -> Json<Value> {
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));

    // Check if session_id is provided (for existing conversations)
    if let Some(session_id) = params.get("session_id").and_then(|v| v.as_str()) {
        let conversations = state.conversations.read().await;

        if let Some(history) = conversations.get(session_id) {
            if history.is_empty() {
                return Json(json!({
                    "success": false,
                    "error": "No messages in conversation"
                }));
            }
            return save_transcript_to_file(history, filename.as_str(), Some(session_id)).await;
        } else {
            return Json(json!({
                "success": false,
                "error": "Conversation not found"
            }));
        }
    }

    // Check if messages array is provided directly
    if let Some(messages) = params.get("messages").and_then(|v| v.as_array()) {
        if messages.is_empty() {
            return Json(json!({
                "success": false,
                "error": "No messages provided"
            }));
        }

        // Convert Value array to ChatMessage vector
        let mut history = Vec::new();
        for msg in messages {
            if let (Some(role), Some(content)) = (
                msg.get("role").and_then(|v| v.as_str()),
                msg.get("content").and_then(|v| v.as_str()),
            ) {
                history.push(op_llm::ChatMessage {
                    role: role.to_string(),
                    content: content.to_string(),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }

        if history.is_empty() {
            return Json(json!({
                "success": false,
                "error": "Invalid message format"
            }));
        }

        return save_transcript_to_file(&history, filename.as_str(), None).await;
    }

    Json(json!({
        "success": false,
        "error": "Either session_id or messages array required",
        "usage": {
            "by_session_id": {"session_id": "session-123", "filename": "optional-filename.txt"},
            "by_messages": {"messages": [{"role": "user", "content": "Hello"}, {"role": "assistant", "content": "Hi"}], "filename": "optional-filename.txt"}
        }
    }))
}

async fn save_transcript_to_file(
    history: &[op_llm::ChatMessage],
    filename: &str,
    session_id: Option<&str>,
) -> Json<Value> {
    // Format transcript
    let mut transcript = String::new();

    if let Some(session) = session_id {
        transcript.push_str(&format!(
            "Chat Transcript - Session: {}
",
            session
        ));
    } else {
        transcript.push_str("Chat Transcript\n");
    }

    transcript.push_str(&format!(
        "Generated: {}
",
        chrono::Utc::now().to_rfc3339()
    ));
    transcript.push_str(&"=".repeat(50));
    transcript.push_str("\n\n");

    for (i, message) in history.iter().enumerate() {
        let role = match message.role.as_str() {
            "user" => "👤 User",
            "assistant" => "🤖 Assistant",
            "system" => "⚙️ System",
            _ => "Unknown",
        };
        transcript.push_str(&format!(
            "[{}] {}
\n",
            role, message.content
        ));
        if i < history.len() - 1 {
            let separator = "─".repeat(30);
            transcript.push_str(&separator);
            transcript.push_str("\n\n");
        }
    }

    // Save to file
    let filepath = format!("/tmp/{}", filename);
    match tokio::fs::write(&filepath, &transcript).await {
        Ok(_) => Json(json!({
            "success": true,
            "message": "Transcript saved successfully",
            "filepath": filepath,
            "filename": filename,
            "message_count": history.len(),
            "transcript_preview": transcript.chars().take(200).collect::<String>() + "..."
        })),
        Err(e) => Json(json!({
            "success": false,
            "error": format!("Failed to save transcript: {}", e)
        })),
    }
}
