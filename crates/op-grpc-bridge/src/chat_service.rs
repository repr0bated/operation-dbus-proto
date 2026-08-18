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
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use op_llm::chat::ChatManager;
use op_llm::ProviderType;
use op_plugins::state_plugins::tched_router::{
    ChatInput, ChatOutput, TchedChatMessage, TchedRouterState,
};
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

#[derive(Clone, Debug)]
struct ResolvedExecutionRoute {
    provider: ProviderType,
    provider_id: String,
    model: String,
    declared_available: bool,
    status_reason: String,
}

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

fn provider_names(state: &TchedRouterState, requested: &str) -> Option<Vec<String>> {
    let requested = requested.trim();
    state
        .catalog
        .providers
        .iter()
        .find(|provider| {
            provider.id.eq_ignore_ascii_case(requested)
                || provider.route.eq_ignore_ascii_case(requested)
                || provider
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(requested))
        })
        .map(|provider| {
            let mut names = vec![provider.id.clone(), provider.route.clone()];
            names.extend(provider.aliases.clone());
            names
        })
}

fn names_contain(names: &[String], candidate: &str) -> bool {
    names
        .iter()
        .any(|name| name.eq_ignore_ascii_case(candidate))
}

fn provider_type_for(state: &TchedRouterState, provider_id: &str) -> anyhow::Result<ProviderType> {
    let names = provider_names(state, provider_id)
        .ok_or_else(|| anyhow!("provider '{provider_id}' is not declared by tched_router"))?;
    names
        .iter()
        .find_map(|name| ProviderType::from_str(name).ok())
        .ok_or_else(|| {
            anyhow!(
                "provider '{}' is declared but has no op-llm runtime adapter",
                names.first().map(String::as_str).unwrap_or(provider_id)
            )
        })
}

fn resolve_declared_route(
    state: &TchedRouterState,
    requested_provider: &str,
    requested_model: &str,
) -> anyhow::Result<ResolvedExecutionRoute> {
    let provider_request = if !requested_provider.trim().is_empty() {
        requested_provider.trim()
    } else {
        state.selected_provider.as_str()
    };
    let model_request = if !requested_model.trim().is_empty() {
        requested_model.trim()
    } else {
        state.selected_model.as_str()
    };

    let provider_aliases = provider_names(state, provider_request)
        .ok_or_else(|| anyhow!("provider '{provider_request}' is not declared by tched_router"))?;

    let provider_matches =
        |route: &&op_plugins::state_plugins::common::llm_projection::ModelRoute| {
            names_contain(&provider_aliases, &route.provider)
                || names_contain(&provider_aliases, &route.upstream_provider)
        };
    let exact_model = |route: &&op_plugins::state_plugins::common::llm_projection::ModelRoute| {
        route.model.eq_ignore_ascii_case(model_request)
    };
    let hint = |route: &&op_plugins::state_plugins::common::llm_projection::ModelRoute| {
        route.hint.eq_ignore_ascii_case(model_request)
    };

    let route = state
        .catalog
        .model_routes
        .iter()
        .filter(provider_matches)
        .find(exact_model)
        .or_else(|| {
            state
                .catalog
                .model_routes
                .iter()
                .filter(provider_matches)
                .find(hint)
        })
        .ok_or_else(|| {
            anyhow!(
                "model or route hint '{model_request}' is not declared for provider '{provider_request}'"
            )
        })?;

    if !matches!(route.kind.as_str(), "chat" | "orchestrator") {
        return Err(anyhow!(
            "route '{}' is kind '{}' and cannot serve chat",
            route.model,
            route.kind
        ));
    }

    let execution_provider = if route.upstream_provider.is_empty() {
        route.provider.as_str()
    } else {
        route.upstream_provider.as_str()
    };

    Ok(ResolvedExecutionRoute {
        provider: provider_type_for(state, execution_provider)?,
        provider_id: execution_provider.to_string(),
        model: route.model.clone(),
        declared_available: route.available,
        status_reason: route.status_reason.clone(),
    })
}

async fn execute_chat(
    chat_manager: &ChatManager,
    state: &TchedRouterState,
    requested_provider: &str,
    requested_model: &str,
    messages: Vec<op_llm::ChatMessage>,
) -> anyhow::Result<(ResolvedExecutionRoute, op_llm::ChatResponse)> {
    let route = resolve_declared_route(state, requested_provider, requested_model)?;
    if !chat_manager.has_provider(&route.provider) {
        return Err(anyhow!(
            "provider '{}' is declared but not configured in the bridge runtime",
            route.provider_id
        ));
    }

    if !route.declared_available {
        let models = chat_manager
            .list_models_for_provider(&route.provider)
            .await
            .with_context(|| {
                format!(
                    "route '{}' is unavailable: {}",
                    route.model, route.status_reason
                )
            })?;
        if route.model != "auto" && !models.iter().any(|model| model.id == route.model) {
            return Err(anyhow!(
                "route '{}' is unavailable: {}",
                route.model,
                route.status_reason
            ));
        }
    }

    let response = chat_manager
        .chat_with(&route.provider, &route.model, messages)
        .await
        .with_context(|| {
            format!(
                "provider '{}' failed model '{}'",
                route.provider_id, route.model
            )
        })?;
    Ok((route, response))
}

/// Execute the schema-declared `tched_router.Chat` method after the mutation
/// engine has recorded the call. Provider/model selection remains owned by
/// the projected tched_router schema; `ChatManager` only performs the resolved
/// upstream call.
pub(crate) async fn dispatch_schema_chat(
    chat_manager: &ChatManager,
    state: &TchedRouterState,
    input: ChatInput,
) -> anyhow::Result<ChatOutput> {
    let messages = if input.messages.is_empty() {
        if input.message.trim().is_empty() {
            return Err(anyhow!("zeroclaw.Chat requires message or messages"));
        }
        vec![op_llm::ChatMessage {
            role: "user".to_string(),
            content: input.message,
            tool_calls: None,
            tool_call_id: None,
        }]
    } else {
        input
            .messages
            .into_iter()
            .map(|message| op_llm::ChatMessage {
                role: message.role,
                content: message.content,
                tool_calls: None,
                tool_call_id: None,
            })
            .collect()
    };

    let (route, response) =
        execute_chat(chat_manager, state, &input.provider, &input.model, messages).await?;

    Ok(ChatOutput {
        content: response.message.content,
        provider: route.provider_id,
        model: route.model,
        finish_reason: response.finish_reason.unwrap_or_else(|| "stop".to_string()),
        usage: serde_json::to_value(response.usage).unwrap_or(serde_json::Value::Null),
    })
}

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
            op_plugins::state_plugins::tched_router::PLUGIN_ID,
            "Chat",
            Some(op_plugins::state_plugins::tched_router::CHAT_CAPABILITY),
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
                .map(|message| TchedChatMessage {
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
                    op_plugins::state_plugins::tched_router::PLUGIN_ID,
                    "Chat",
                    &chat_args,
                    Some(op_plugins::state_plugins::tched_router::CHAT_CAPABILITY),
                    &actor_id,
                ) => result.and_then(|result| {
                    let payload = result
                        .get("result")
                        .cloned()
                        .ok_or_else(|| anyhow!("tched_router.Chat returned no result payload"))?;
                    serde_json::from_value::<ChatOutput>(payload)
                        .context("invalid tched_router.Chat result")
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

#[cfg(test)]
mod tests {
    use super::*;
    use op_plugins::state_plugins::tched_router::TchedRouterPlugin;

    #[test]
    fn explicit_provider_and_model_resolve_through_schema_catalog() {
        let mut state = TchedRouterPlugin::current_state();
        let route = state
            .catalog
            .model_routes
            .iter_mut()
            .find(|route| route.provider == "salad" && route.hint == "balanced")
            .expect("balanced Salad route");
        route.available = true;

        let resolved =
            resolve_declared_route(&state, "salad", "qwen3.6-27b").expect("route resolves");
        assert_eq!(resolved.provider, ProviderType::Salad);
        assert_eq!(resolved.provider_id, "salad");
        assert_eq!(resolved.model, "qwen3.6-27b");
        assert!(resolved.declared_available);
    }

    #[test]
    fn provider_alias_and_route_hint_resolve() {
        let state = TchedRouterPlugin::current_state();
        let resolved =
            resolve_declared_route(&state, "salad-ai", "fast").expect("alias and hint resolve");
        assert_eq!(resolved.provider, ProviderType::Salad);
        assert_eq!(resolved.model, "qwen3.5-9b");
    }

    #[test]
    fn selected_provider_and_model_are_the_default_route() {
        let mut state = TchedRouterPlugin::current_state();
        state.selected_provider = "salad".to_string();
        state.selected_model = "qwen3.6-35b-a3b".to_string();

        let resolved = resolve_declared_route(&state, "", "").expect("selected route resolves");
        assert_eq!(resolved.provider, ProviderType::Salad);
        assert_eq!(resolved.model, "qwen3.6-35b-a3b");
    }

    #[test]
    fn undeclared_model_fails_closed() {
        let state = TchedRouterPlugin::current_state();
        let error = resolve_declared_route(&state, "salad", "not-a-model").unwrap_err();
        assert!(error.to_string().contains("not declared"));
    }

    #[test]
    fn non_chat_routes_fail_closed() {
        let state = TchedRouterPlugin::current_state();
        let error = resolve_declared_route(&state, "oscal", "compliance").unwrap_err();
        assert!(error.to_string().contains("cannot serve chat"));
    }
}
