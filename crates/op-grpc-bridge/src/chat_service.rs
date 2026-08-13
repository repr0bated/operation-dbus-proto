//! ChatService gRPC server implementation.
//!
//! Implements the `op_chat.chat.ChatService` trait from `chat.proto`.
//! Served on the op-grpc-bridge alongside StateSync, PluginService, etc.
//! so zeroclaw-gui discovers it via a single reflection endpoint.
//!
//! Architecture:
//! - zeroclaw owns provider/model routing (OD-28) — SendRequest carries them.
//! - gemma_brain routes to the selected model and applies compliance tags.
//! - The chatbot is a DELEGATOR — forced tool calling is mandatory; even user
//!   responses are emitted as tool calls (respond_to_user).
//! - Chat persistence flows through the memory loop → cognitive-mcp.
//! - The agent loop is bounded (≥50 steps) and cancellable.

use std::pin::Pin;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use op_plugins::state_plugins::zeroclaw::{ChatInput, ChatOutput, ZeroclawChatMessage};
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::interceptor::GhostbridgeIdentity;
use crate::mutation_engine::MutationEngine;
use crate::proto::chat::{
    chat_frame, chat_service_server::ChatService, ApproveRequest, ApproveResponse, CancelRequest,
    CancelResponse, ChatFrame, Heartbeat, SendRequest, StreamDone, StreamError, UiMessagePart,
};

/// Shared state for the ChatService.
pub struct ChatServiceImpl {
    /// Active conversation cursors for cancel support.
    cancellations: Arc<Mutex<std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>>,
    /// Pending approvals: tool_call_id -> oneshot sender.
    approvals: Arc<Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    /// The sole schema method/event-chain authority.
    engine: Arc<MutationEngine>,
}

impl ChatServiceImpl {
    pub fn new(engine: Arc<MutationEngine>) -> Self {
        Self {
            cancellations: Arc::new(Mutex::new(Default::default())),
            approvals: Arc::new(Mutex::new(Default::default())),
            engine,
        }
    }
}

type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatFrame, Status>> + Send + 'static>>;

#[async_trait::async_trait]
impl ChatService for ChatServiceImpl {
    type SendStream = ChatStream;

    async fn send(
        &self,
        request: Request<SendRequest>,
    ) -> Result<Response<Self::SendStream>, Status> {
        let identity = request
            .extensions()
            .get::<GhostbridgeIdentity>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Ghostbridge identity is required"))?;
        crate::grpc_server::authorize_schema_method(
            "zeroclaw",
            "Chat",
            Some("cap.software.zeroclaw.chat@v1"),
            Some(&identity),
        )?;
        let req = request.into_inner();
        let conversation_id = req.conversation_id.clone();
        let provider = req.provider.clone();
        let model = req.model.clone();

        info!(
            conversation_id = %conversation_id,
            provider = %provider,
            model = %model,
            "ChatService.Send — streaming chat completion"
        );

        // Parse ui_messages from JSON bytes.
        let ui_messages: Vec<serde_json::Value> = serde_json::from_slice(&req.ui_messages)
            .map_err(|e| Status::invalid_argument(format!("Invalid ui_messages JSON: {e}")))?;
        let chat_args = ChatInput {
            message: String::new(),
            messages: ui_messages
                .iter()
                .map(|message| ZeroclawChatMessage {
                    role: message
                        .get("role")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("user")
                        .to_string(),
                    content: message
                        .get("content")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                })
                .collect(),
            provider,
            model,
        };
        let chat_args = serde_json::to_string(&chat_args)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;

        // Set up cancellation channel.
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
        {
            let mut cancellations = self.cancellations.lock().await;
            cancellations.insert(conversation_id.clone(), cancel_tx);
        }

        // Cursor for monotonic frame ordering.
        let cursor = std::sync::atomic::AtomicU64::new(0);

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<ChatFrame, Status>>(64);

        let cancellations = self.cancellations.clone();
        let engine = self.engine.clone();
        let actor_id = identity.session_id;
        let conv_id = conversation_id.clone();

        tokio::spawn(async move {
            let bump = || cursor.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Emit heartbeat immediately so the client knows the stream is alive.
            let _ = tx
                .send(Ok(ChatFrame {
                    cursor: bump(),
                    body: Some(chat_frame::Body::Heartbeat(Heartbeat {
                        server_time_ms: chrono::Utc::now().timestamp_millis() as u64,
                    })),
                }))
                .await;

            let completion = tokio::select! {
                result = engine.dispatch_method_call(
                    "zeroclaw",
                    "Chat",
                    &chat_args,
                    Some("cap.software.zeroclaw.chat@v1"),
                    &actor_id,
                ) => result.and_then(|result| {
                    let payload = result
                        .get("result")
                        .cloned()
                        .ok_or_else(|| anyhow!("zeroclaw.Chat returned no result payload"))?;
                    serde_json::from_value::<ChatOutput>(payload)
                        .context("invalid zeroclaw.Chat result")
                }),
                changed = cancel_rx.changed() => {
                    match changed {
                        Ok(()) if *cancel_rx.borrow() => Err(anyhow!("chat cancelled")),
                        Ok(()) => Err(anyhow!("chat cancellation channel changed unexpectedly")),
                        Err(_) => Err(anyhow!("chat cancellation channel closed")),
                    }
                }
            };

            let total_parts = match completion {
                Ok(response) => {
                    let payload = serde_json::json!({
                        "type": "text",
                        "text": response.content,
                        "provider": response.provider,
                        "model": response.model,
                    });
                    let _ = tx
                        .send(Ok(ChatFrame {
                            cursor: bump(),
                            body: Some(chat_frame::Body::Part(UiMessagePart {
                                message_id: uuid::Uuid::new_v4().to_string(),
                                role: "assistant".to_string(),
                                kind: "text".to_string(),
                                payload: serde_json::to_vec(&payload).unwrap_or_default(),
                            })),
                        }))
                        .await;
                    1
                }
                Err(error) => {
                    let cancelled = error.to_string().contains("cancelled");
                    let _ = tx
                        .send(Ok(ChatFrame {
                            cursor: bump(),
                            body: Some(chat_frame::Body::Error(StreamError {
                                code: if cancelled {
                                    "cancelled".to_string()
                                } else {
                                    "route_unavailable".to_string()
                                },
                                message: error.to_string(),
                                retryable: false,
                                retry_after_ms: None,
                            })),
                        }))
                        .await;
                    0
                }
            };

            // Stream done.
            let _ = tx
                .send(Ok(ChatFrame {
                    cursor: bump(),
                    body: Some(chat_frame::Body::Done(StreamDone {
                        conversation_id: conv_id.clone(),
                        total_parts,
                    })),
                }))
                .await;

            // Cleanup cancellation registration.
            {
                let mut cancellations = cancellations.lock().await;
                cancellations.remove(&conv_id);
            }
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn approve(
        &self,
        request: Request<ApproveRequest>,
    ) -> Result<Response<ApproveResponse>, Status> {
        let req = request.into_inner();
        info!(
            conversation_id = %req.conversation_id,
            tool_call_id = %req.tool_call_id,
            approved = req.approved,
            "ChatService.Approve"
        );

        // Deliver the approval decision to the pending tool call.
        let mut approvals = self.approvals.lock().await;
        match approvals.remove(&req.tool_call_id) {
            Some(sender) => {
                let _ = sender.send(req.approved);
                Ok(Response::new(ApproveResponse {
                    success: true,
                    tool_call_id: req.tool_call_id,
                    error: None,
                }))
            }
            None => {
                warn!(
                    tool_call_id = %req.tool_call_id,
                    "Approval requested for unknown tool call"
                );
                Ok(Response::new(ApproveResponse {
                    success: false,
                    tool_call_id: req.tool_call_id,
                    error: Some("Tool call not found or already completed".to_string()),
                }))
            }
        }
    }

    async fn cancel(
        &self,
        request: Request<CancelRequest>,
    ) -> Result<Response<CancelResponse>, Status> {
        let req = request.into_inner();
        info!(
            conversation_id = %req.conversation_id,
            "ChatService.Cancel"
        );

        let mut cancellations = self.cancellations.lock().await;
        match cancellations.remove(&req.conversation_id) {
            Some(cancel_tx) => {
                let _ = cancel_tx.send(true);
                Ok(Response::new(CancelResponse {
                    success: true,
                    conversation_id: req.conversation_id,
                }))
            }
            None => {
                warn!(
                    conversation_id = %req.conversation_id,
                    "Cancel requested for unknown conversation"
                );
                Ok(Response::new(CancelResponse {
                    success: false,
                    conversation_id: req.conversation_id,
                }))
            }
        }
    }
}
