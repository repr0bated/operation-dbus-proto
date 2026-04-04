//! gRPC Client - For D-Bus → remote gRPC calls
//!
//! Allows local D-Bus services to call remote gRPC endpoints,
//! enabling distributed operation-dbus deployments.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use prost_types::{value::Kind as ProstKind, Struct as ProstStruct, Value as ProstValue};
use tokio::sync::RwLock;
use tonic::transport::{Channel, Endpoint};
use tracing::info;

use crate::proto::{
    event_chain_service_client::EventChainServiceClient,
    plugin_service_client::PluginServiceClient, state_sync_client::StateSyncClient,
    CallMethodRequest, GetStateRequest, MutateRequest, OperationType as ProtoOperationType,
    SubscribeEventsRequest, SubscribeRequest,
};

/// Configuration for a remote gRPC endpoint
#[derive(Debug, Clone)]
pub struct RemoteEndpoint {
    pub address: String,
    pub tls_enabled: bool,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

impl Default for RemoteEndpoint {
    fn default() -> Self {
        Self {
            address: "http://127.0.0.1:50051".to_string(),
            tls_enabled: false,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// gRPC client pool for connecting to remote Operation services
pub struct GrpcClientPool {
    /// Map of endpoint address to channel
    channels: RwLock<HashMap<String, Channel>>,
    /// Default endpoint configuration
    default_config: RemoteEndpoint,
}

impl GrpcClientPool {
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            default_config: RemoteEndpoint::default(),
        }
    }

    pub fn with_default_config(config: RemoteEndpoint) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            default_config: config,
        }
    }

    /// Get or create a channel to the specified endpoint
    async fn get_channel(&self, address: &str) -> Result<Channel, GrpcClientError> {
        {
            let channels = self.channels.read().await;
            if let Some(channel) = channels.get(address) {
                return Ok(channel.clone());
            }
        }

        let endpoint = Endpoint::from_shared(address.to_string())
            .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?
            .connect_timeout(self.default_config.connect_timeout)
            .timeout(self.default_config.request_timeout);

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?;

        {
            let mut channels = self.channels.write().await;
            channels.insert(address.to_string(), channel.clone());
        }

        info!("Connected to remote gRPC endpoint: {}", address);
        Ok(channel)
    }

    /// Get a Plugin service client
    pub async fn plugin_service_client(
        &self,
        address: &str,
    ) -> Result<PluginServiceClient<Channel>, GrpcClientError> {
        let channel = self.get_channel(address).await?;
        Ok(PluginServiceClient::new(channel))
    }

    /// Get a StateSync service client
    pub async fn state_sync_client(
        &self,
        address: &str,
    ) -> Result<StateSyncClient<Channel>, GrpcClientError> {
        let channel = self.get_channel(address).await?;
        Ok(StateSyncClient::new(channel))
    }

    /// Get an EventChain service client
    pub async fn event_chain_client(
        &self,
        address: &str,
    ) -> Result<EventChainServiceClient<Channel>, GrpcClientError> {
        let channel = self.get_channel(address).await?;
        Ok(EventChainServiceClient::new(channel))
    }

    /// Close all connections
    pub async fn close_all(&self) {
        let mut channels = self.channels.write().await;
        channels.clear();
        info!("Closed all gRPC client connections");
    }
}

/// High-level client for remote Operation services
pub struct RemoteOperationClient {
    pool: Arc<GrpcClientPool>,
    default_address: String,
    client_id: String,
}

impl RemoteOperationClient {
    pub fn new(pool: Arc<GrpcClientPool>, address: &str, client_id: &str) -> Self {
        Self {
            pool,
            default_address: address.to_string(),
            client_id: client_id.to_string(),
        }
    }

    /// Get state from a remote endpoint
    pub async fn get_state(
        &self,
        plugin_id: &str,
        object_path: &str,
    ) -> Result<simd_json::OwnedValue, GrpcClientError> {
        let mut client = self.pool.state_sync_client(&self.default_address).await?;

        let request = tonic::Request::new(GetStateRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
        });

        let response = client
            .get_state(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let resp = response.into_inner();
        let state = resp.state.unwrap_or_default();
        Ok(prost_struct_to_simd(&state))
    }

    /// Set state on a remote endpoint (apply patch)
    pub async fn set_state(
        &self,
        plugin_id: &str,
        object_path: &str,
        state: simd_json::OwnedValue,
        actor_id: &str,
        capability_id: &str,
    ) -> Result<SetStateResult, GrpcClientError> {
        let mut client = self.pool.state_sync_client(&self.default_address).await?;

        let request = tonic::Request::new(MutateRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
            operation: ProtoOperationType::ApplyPatch as i32,
            member_name: String::new(),
            value: Some(simd_to_prost_value(&state)),
            actor_id: actor_id.to_string(),
            capability_id: capability_id.to_string(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
        });

        let response = client
            .mutate(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let resp = response.into_inner();
        if !resp.success {
            if let Some(err) = resp.error {
                return Err(GrpcClientError::RemoteError {
                    code: format!("{}", err.code),
                    message: err.message,
                });
            }
            return Err(GrpcClientError::RemoteError {
                code: "UNKNOWN".to_string(),
                message: "mutation failed".to_string(),
            });
        }

        Ok(SetStateResult {
            event_id: resp.event_id,
            effective_hash: resp.effective_hash,
        })
    }

    /// Call a method on a remote endpoint
    pub async fn call_method(
        &self,
        plugin_id: &str,
        object_path: &str,
        interface_name: &str,
        method_name: &str,
        arguments: Vec<simd_json::OwnedValue>,
        actor_id: &str,
        capability_id: &str,
    ) -> Result<simd_json::OwnedValue, GrpcClientError> {
        let mut client = self
            .pool
            .plugin_service_client(&self.default_address)
            .await?;

        let arguments = arguments
            .iter()
            .map(simd_to_prost_value)
            .collect::<Vec<_>>();

        let request = tonic::Request::new(CallMethodRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
            interface_name: interface_name.to_string(),
            method_name: method_name.to_string(),
            arguments,
            actor_id: actor_id.to_string(),
            capability_id: capability_id.to_string(),
        });

        let response = client
            .call_method(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let resp = response.into_inner();
        if !resp.success {
            if let Some(err) = resp.error {
                return Err(GrpcClientError::RemoteError {
                    code: format!("{}", err.code),
                    message: err.message,
                });
            }
            return Err(GrpcClientError::RemoteError {
                code: "UNKNOWN".to_string(),
                message: "call failed".to_string(),
            });
        }

        if let Some(result) = resp.result {
            Ok(prost_value_to_simd(&result))
        } else {
            Ok(simd_json::json!(null))
        }
    }

    /// Subscribe to state updates from a remote endpoint
    pub async fn subscribe(
        &self,
        plugin_filters: Vec<String>,
        path_filters: Vec<String>,
        tag_filters: Vec<String>,
    ) -> Result<
        impl tokio_stream::Stream<Item = Result<StateUpdateMessage, GrpcClientError>>,
        GrpcClientError,
    > {
        let mut client = self.pool.state_sync_client(&self.default_address).await?;

        let request = tonic::Request::new(SubscribeRequest {
            plugin_ids: plugin_filters,
            path_patterns: path_filters,
            tags: tag_filters,
            include_initial_state: false,
        });

        let response = client
            .subscribe(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let stream = response.into_inner();

        Ok(tokio_stream::StreamExt::map(stream, |result| {
            result
                .map(|update| StateUpdateMessage {
                    plugin_id: update.plugin_id,
                    object_path: update.object_path,
                    property_name: if update.member_name.is_empty() {
                        None
                    } else {
                        Some(update.member_name)
                    },
                    new_value: update.new_value.as_ref().map(prost_value_to_simd),
                    event_id: update.event_id.to_string(),
                    tags_touched: update.tags_touched,
                })
                .map_err(|e| GrpcClientError::StreamError(e.to_string()))
        }))
    }

    /// Subscribe to chain events from a remote endpoint
    pub async fn stream_events(
        &self,
        from_event_id: Option<u64>,
        plugin_filters: Vec<String>,
        tag_filters: Vec<String>,
    ) -> Result<
        impl tokio_stream::Stream<Item = Result<ChainEventMessage, GrpcClientError>>,
        GrpcClientError,
    > {
        let mut client = self.pool.event_chain_client(&self.default_address).await?;

        let request = tonic::Request::new(SubscribeEventsRequest {
            from_event_id: from_event_id.unwrap_or_default(),
            plugin_id: plugin_filters.first().cloned().unwrap_or_default(),
            tags: tag_filters,
        });

        let response = client
            .subscribe_events(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let stream = response.into_inner();

        Ok(tokio_stream::StreamExt::map(stream, |result| {
            result
                .map(|event| ChainEventMessage {
                    event_id: event.event_id.to_string(),
                    event_hash: event.event_hash,
                    prev_hash: event.prev_hash,
                    plugin_id: event.plugin_id,
                    operation_type: event.operation_type,
                    target: event.target,
                    decision: event.decision.to_string(),
                    tags_touched: event.tags_touched,
                })
                .map_err(|e| GrpcClientError::StreamError(e.to_string()))
        }))
    }
}

/// Result of a set state operation
#[derive(Debug, Clone)]
pub struct SetStateResult {
    pub event_id: u64,
    pub effective_hash: String,
}

/// State update message from subscription
#[derive(Debug, Clone)]
pub struct StateUpdateMessage {
    pub plugin_id: String,
    pub object_path: String,
    pub property_name: Option<String>,
    pub new_value: Option<simd_json::OwnedValue>,
    pub event_id: String,
    pub tags_touched: Vec<String>,
}

/// Chain event message from event stream
#[derive(Debug, Clone)]
pub struct ChainEventMessage {
    pub event_id: String,
    pub event_hash: String,
    pub prev_hash: String,
    pub plugin_id: String,
    pub operation_type: String,
    pub target: String,
    pub decision: String,
    pub tags_touched: Vec<String>,
}

/// Errors that can occur in gRPC client operations
#[derive(Debug, Clone)]
pub enum GrpcClientError {
    ConnectionFailed(String),
    RequestFailed(String),
    StreamError(String),
    ParseError(String),
    RemoteError { code: String, message: String },
}

impl std::fmt::Display for GrpcClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            Self::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            Self::StreamError(msg) => write!(f, "Stream error: {}", msg),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::RemoteError { code, message } => {
                write!(f, "Remote error [{}]: {}", code, message)
            }
        }
    }
}

impl std::error::Error for GrpcClientError {}

fn prost_value_to_simd(value: &ProstValue) -> simd_json::OwnedValue {
    let serde_value = prost_value_to_serde(value);
    simd_json::serde::to_owned_value(&serde_value).unwrap_or_else(|_| simd_json::json!(null))
}

fn prost_struct_to_simd(value: &ProstStruct) -> simd_json::OwnedValue {
    let serde_value = serde_json::Value::Object(
        value
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), prost_value_to_serde(v)))
            .collect(),
    );
    simd_json::serde::to_owned_value(&serde_value).unwrap_or_else(|_| simd_json::json!(null))
}

fn prost_value_to_serde(value: &ProstValue) -> serde_json::Value {
    match &value.kind {
        None => serde_json::Value::Null,
        Some(ProstKind::NullValue(_)) => serde_json::Value::Null,
        Some(ProstKind::BoolValue(b)) => serde_json::Value::Bool(*b),
        Some(ProstKind::NumberValue(n)) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Some(ProstKind::StringValue(s)) => serde_json::Value::String(s.clone()),
        Some(ProstKind::StructValue(s)) => serde_json::Value::Object(
            s.fields
                .iter()
                .map(|(k, v)| (k.clone(), prost_value_to_serde(v)))
                .collect(),
        ),
        Some(ProstKind::ListValue(l)) => {
            serde_json::Value::Array(l.values.iter().map(prost_value_to_serde).collect())
        }
    }
}

fn simd_to_prost_value(value: &simd_json::OwnedValue) -> ProstValue {
    let json = simd_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    let serde_value: serde_json::Value =
        serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
    serde_to_prost_value(&serde_value)
}

fn serde_to_prost_value(value: &serde_json::Value) -> ProstValue {
    match value {
        serde_json::Value::Null => ProstValue {
            kind: Some(ProstKind::NullValue(0)),
        },
        serde_json::Value::Bool(b) => ProstValue {
            kind: Some(ProstKind::BoolValue(*b)),
        },
        serde_json::Value::Number(n) => ProstValue {
            kind: Some(ProstKind::NumberValue(n.as_f64().unwrap_or(0.0))),
        },
        serde_json::Value::String(s) => ProstValue {
            kind: Some(ProstKind::StringValue(s.clone())),
        },
        serde_json::Value::Array(arr) => ProstValue {
            kind: Some(ProstKind::ListValue(prost_types::ListValue {
                values: arr.iter().map(serde_to_prost_value).collect(),
            })),
        },
        serde_json::Value::Object(map) => ProstValue {
            kind: Some(ProstKind::StructValue(ProstStruct {
                fields: map
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_to_prost_value(v)))
                    .collect(),
            })),
        },
    }
}
