//! EMQX ExHook v2 HookProvider gRPC service implementation.
//!
//! EMQX acts as a gRPC client, calling this service for MQTT events
//! (EMQX 5.x fixed the exhook proto package at `emqx.exhook.v2`).
//! Authenticate/authorize/publish decisions are left to the broker
//! (`IGNORE`) — the hook is an audit tap, not the auth gate; identity
//! gating stays with the Ghostbridge interceptor on the plugin surface.
//!
//! These RPCs stay here. The `emqx` plugin is present-state only.

use std::sync::Arc;

use tonic::{Request, Response, Status};
use tracing::{debug, info};

use crate::mutation_engine::MutationEngine;
use crate::proto::emqx_exhook::{
    self as exhook, hook_provider_server::HookProvider, valued_response, EmptySuccess, HookSpec,
    LoadedResponse, ValuedResponse,
};

/// Hooks registered with the broker on OnProviderLoaded.
const REGISTERED_HOOKS: &[&str] = &[
    "client.connect",
    "client.connected",
    "client.disconnected",
    "client.authenticate",
    "client.authorize",
    "client.subscribe",
    "client.unsubscribe",
    "session.created",
    "session.subscribed",
    "session.unsubscribed",
    "session.terminated",
    "message.publish",
];

/// HookProvider service implementation for EMQX MQTT event hooks.
#[derive(Clone)]
pub struct HookProviderService {
    _mutation_engine: Arc<MutationEngine>,
}

impl HookProviderService {
    pub fn new(mutation_engine: Arc<MutationEngine>) -> Self {
        Self {
            _mutation_engine: mutation_engine,
        }
    }
}

fn client_id(info: &Option<exhook::ClientInfo>) -> &str {
    info.as_ref()
        .map(|c| c.clientid.as_str())
        .unwrap_or("unknown")
}

fn empty() -> Response<EmptySuccess> {
    Response::new(EmptySuccess {})
}

/// Leave the broker's own decision in place.
fn ignore() -> Response<ValuedResponse> {
    Response::new(ValuedResponse {
        r#type: valued_response::ResponsedType::Ignore as i32,
        value: None,
    })
}

#[tonic::async_trait]
impl HookProvider for HookProviderService {
    async fn on_provider_loaded(
        &self,
        request: Request<exhook::ProviderLoadedRequest>,
    ) -> Result<Response<LoadedResponse>, Status> {
        let req = request.into_inner();
        info!(
            broker = %req.broker.as_ref().map(|b| b.version.as_str()).unwrap_or("unknown"),
            "EMQX exhook provider loaded"
        );
        let hooks = REGISTERED_HOOKS
            .iter()
            .map(|name| HookSpec {
                name: (*name).to_string(),
                topics: if name.starts_with("message.") {
                    vec!["#".to_string()]
                } else {
                    Vec::new()
                },
            })
            .collect();
        Ok(Response::new(LoadedResponse { hooks }))
    }

    async fn on_provider_unloaded(
        &self,
        _request: Request<exhook::ProviderUnloadedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        info!("EMQX exhook provider unloaded");
        Ok(empty())
    }

    async fn on_client_connect(
        &self,
        request: Request<exhook::ClientConnectRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        let client = req
            .conninfo
            .as_ref()
            .map(|c| c.clientid.as_str())
            .unwrap_or("unknown");
        debug!(client_id = %client, "EMQX OnClientConnect");
        Ok(empty())
    }

    async fn on_client_connack(
        &self,
        _request: Request<exhook::ClientConnackRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        Ok(empty())
    }

    async fn on_client_connected(
        &self,
        request: Request<exhook::ClientConnectedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        info!(client_id = %client_id(&req.clientinfo), "EMQX OnClientConnected");
        Ok(empty())
    }

    async fn on_client_disconnected(
        &self,
        request: Request<exhook::ClientDisconnectedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        info!(
            client_id = %client_id(&req.clientinfo),
            reason = %req.reason,
            "EMQX OnClientDisconnected"
        );
        Ok(empty())
    }

    async fn on_client_authenticate(
        &self,
        request: Request<exhook::ClientAuthenticateRequest>,
    ) -> Result<Response<ValuedResponse>, Status> {
        let req = request.into_inner();
        debug!(
            client_id = %client_id(&req.clientinfo),
            result = req.result,
            "EMQX OnClientAuthenticate"
        );
        Ok(ignore())
    }

    async fn on_client_authorize(
        &self,
        request: Request<exhook::ClientAuthorizeRequest>,
    ) -> Result<Response<ValuedResponse>, Status> {
        let req = request.into_inner();
        debug!(
            client_id = %client_id(&req.clientinfo),
            topic = %req.topic,
            "EMQX OnClientAuthorize"
        );
        Ok(ignore())
    }

    async fn on_client_subscribe(
        &self,
        request: Request<exhook::ClientSubscribeRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        debug!(
            client_id = %client_id(&req.clientinfo),
            "EMQX OnClientSubscribe"
        );
        Ok(empty())
    }

    async fn on_client_unsubscribe(
        &self,
        request: Request<exhook::ClientUnsubscribeRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        debug!(
            client_id = %client_id(&req.clientinfo),
            "EMQX OnClientUnsubscribe"
        );
        Ok(empty())
    }

    async fn on_session_created(
        &self,
        request: Request<exhook::SessionCreatedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        debug!(
            client_id = %client_id(&req.clientinfo),
            "EMQX OnSessionCreated"
        );
        Ok(empty())
    }

    async fn on_session_subscribed(
        &self,
        request: Request<exhook::SessionSubscribedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        debug!(
            client_id = %client_id(&req.clientinfo),
            topic = %req.topic,
            "EMQX OnSessionSubscribed"
        );
        Ok(empty())
    }

    async fn on_session_unsubscribed(
        &self,
        request: Request<exhook::SessionUnsubscribedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        debug!(
            client_id = %client_id(&req.clientinfo),
            topic = %req.topic,
            "EMQX OnSessionUnsubscribed"
        );
        Ok(empty())
    }

    async fn on_session_resumed(
        &self,
        _request: Request<exhook::SessionResumedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        Ok(empty())
    }

    async fn on_session_discarded(
        &self,
        _request: Request<exhook::SessionDiscardedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        Ok(empty())
    }

    async fn on_session_takenover(
        &self,
        _request: Request<exhook::SessionTakenoverRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        Ok(empty())
    }

    async fn on_session_terminated(
        &self,
        request: Request<exhook::SessionTerminatedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        let req = request.into_inner();
        debug!(
            client_id = %client_id(&req.clientinfo),
            reason = %req.reason,
            "EMQX OnSessionTerminated"
        );
        Ok(empty())
    }

    async fn on_message_publish(
        &self,
        request: Request<exhook::MessagePublishRequest>,
    ) -> Result<Response<ValuedResponse>, Status> {
        let req = request.into_inner();
        if let Some(message) = req.message.as_ref() {
            info!(from = %message.from, topic = %message.topic, "EMQX OnMessagePublish");
        }
        Ok(ignore())
    }

    async fn on_message_delivered(
        &self,
        _request: Request<exhook::MessageDeliveredRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        Ok(empty())
    }

    async fn on_message_dropped(
        &self,
        _request: Request<exhook::MessageDroppedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        Ok(empty())
    }

    async fn on_message_acked(
        &self,
        _request: Request<exhook::MessageAckedRequest>,
    ) -> Result<Response<EmptySuccess>, Status> {
        Ok(empty())
    }
}
