//! gRPC Client - For D-Bus → remote gRPC calls
//!
//! Allows local D-Bus services to call remote gRPC endpoints,
//! enabling distributed operation-dbus deployments.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use prost_types::{value::Kind as ProstKind, Struct as ProstStruct, Value as ProstValue};
use tokio::sync::RwLock;
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint};
use tonic::Request;
use tracing::info;

use crate::proto::{
    event_chain_service_client::EventChainServiceClient,
    plugin_service_client::PluginServiceClient, state_sync_client::StateSyncClient,
    CallMethodRequest, GetStateRequest, MutateRequest, OperationType as ProtoOperationType,
    SubscribeEventsRequest, SubscribeRequest,
};
use op_grpc_adapters::proto::{
    execute_command_request::Command as ExecuteNetclientCommand,
    netmaker_service_client::NetmakerServiceClient, ConnectRequest, DisconnectRequest,
    ExecuteCommandRequest, ExecuteCommandResponse, InstallRequest, JoinNetworkRequest,
    LeaveNetworkRequest, ListRequest, PeersRequest, PingRequest, PullRequest, PushRequest,
    RegisterRequest, RestartServiceRequest, ServerRequest, UninstallRequest, UseRequest,
    VersionRequest,
};

/// Configuration for a remote gRPC endpoint
#[derive(Debug, Clone)]
pub struct RemoteEndpoint {
    pub address: String,
    pub tls_enabled: bool,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

/// Caller-supplied Ghostbridge identity forwarded by an HTTP compatibility
/// adapter into the canonical gRPC method call.
#[derive(Debug, Clone)]
pub struct GhostbridgeCallMetadata {
    pub footprint: String,
    pub trace_id: Option<String>,
    pub wireguard_pubkey: Option<String>,
}

impl Default for RemoteEndpoint {
    fn default() -> Self {
        Self {
            address: "https://localhost:8090".to_string(),
            tls_enabled: true,
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

impl Default for GrpcClientPool {
    fn default() -> Self {
        Self::new()
    }
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

    fn configure_endpoint(
        &self,
        address: &str,
        endpoint: Endpoint,
    ) -> Result<Endpoint, GrpcClientError> {
        let endpoint = endpoint
            .connect_timeout(self.default_config.connect_timeout)
            .timeout(self.default_config.request_timeout);

        if !address.starts_with("https://") {
            return Ok(endpoint);
        }

        let mut tls = ClientTlsConfig::new().with_native_roots();
        if let Ok(path) = std::env::var("OP_DBUS_GRPC_CA_FILE") {
            if !path.trim().is_empty() {
                let pem = std::fs::read(&path).map_err(|error| {
                    GrpcClientError::ConnectionFailed(format!(
                        "failed to read gRPC CA certificate {path}: {error}"
                    ))
                })?;
                tls = tls.ca_certificate(Certificate::from_pem(pem));
            }
        }
        if let Ok(domain) = std::env::var("OP_DBUS_GRPC_TLS_DOMAIN") {
            if !domain.trim().is_empty() {
                tls = tls.domain_name(domain);
            }
        }

        endpoint
            .tls_config(tls)
            .map_err(|error| GrpcClientError::ConnectionFailed(error.to_string()))
    }

    /// Get or create a channel to the specified address (supports comma-separated endpoints for load balancing)
    async fn get_channel(&self, address: &str) -> Result<Channel, GrpcClientError> {
        {
            let channels = self.channels.read().await;
            if let Some(channel) = channels.get(address) {
                return Ok(channel.clone());
            }
        }

        let addrs: Vec<&str> = address
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let channel = if addrs.len() > 1 {
            // Native Tonic Load Balancing
            let endpoints = addrs
                .into_iter()
                .map(|addr| {
                    let endpoint = Endpoint::from_shared(addr.to_string()).map_err(|error| {
                        GrpcClientError::ConnectionFailed(format!(
                            "Invalid endpoint {addr}: {error}"
                        ))
                    })?;
                    self.configure_endpoint(addr, endpoint)
                })
                .collect::<Result<Vec<_>, _>>()?;

            Channel::balance_list(endpoints.into_iter())
        } else {
            // Single endpoint
            let endpoint = Endpoint::from_shared(address.to_string())
                .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?;
            let endpoint = self.configure_endpoint(address, endpoint)?;

            endpoint
                .connect()
                .await
                .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?
        };

        {
            let mut channels = self.channels.write().await;
            channels.insert(address.to_string(), channel.clone());
        }

        info!("Connected to remote gRPC endpoint(s): {}", address);
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

    /// Get a Netmaker adapter client.
    pub async fn netmaker_client(
        &self,
        address: &str,
    ) -> Result<NetmakerServiceClient<Channel>, GrpcClientError> {
        let channel = self.get_channel(address).await?;
        Ok(NetmakerServiceClient::new(channel))
    }

    /// Close all connections
    pub async fn close_all(&self) {
        let mut channels = self.channels.write().await;
        channels.clear();
        info!("Closed all gRPC client connections");
    }
}

/// High-level client for remote Operation services
#[allow(dead_code)]
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

        let mut request = Request::new(GetStateRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
        });
        attach_ghostbridge_metadata(&mut request)?;

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

        let mut request = Request::new(MutateRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
            operation: ProtoOperationType::ApplyPatch as i32,
            member_name: String::new(),
            value: Some(simd_to_prost_value(&state)),
            actor_id: actor_id.to_string(),
            capability_id: capability_id.to_string(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
        });
        attach_ghostbridge_metadata(&mut request)?;

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
    #[allow(clippy::too_many_arguments)]
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

        let mut request = Request::new(CallMethodRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
            interface_name: interface_name.to_string(),
            method_name: method_name.to_string(),
            arguments,
            actor_id: actor_id.to_string(),
            capability_id: capability_id.to_string(),
        });
        attach_ghostbridge_metadata(&mut request)?;

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

    /// Call a method while preserving the Ghostbridge identity supplied by an
    /// outer transport adapter. This keeps HTTP on op-web while capability
    /// enforcement remains in the bridge's canonical gRPC method pipeline.
    #[allow(clippy::too_many_arguments)]
    pub async fn call_method_with_metadata(
        &self,
        plugin_id: &str,
        object_path: &str,
        interface_name: &str,
        method_name: &str,
        arguments: Vec<simd_json::OwnedValue>,
        actor_id: &str,
        capability_id: &str,
        identity: &GhostbridgeCallMetadata,
    ) -> Result<simd_json::OwnedValue, GrpcClientError> {
        let mut client = self
            .pool
            .plugin_service_client(&self.default_address)
            .await?;

        let arguments = arguments
            .iter()
            .map(simd_to_prost_value)
            .collect::<Vec<_>>();

        let mut request = Request::new(CallMethodRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
            interface_name: interface_name.to_string(),
            method_name: method_name.to_string(),
            arguments,
            actor_id: actor_id.to_string(),
            capability_id: capability_id.to_string(),
        });
        attach_supplied_ghostbridge_metadata(&mut request, identity)?;

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
    /// Subscribe to state changes.
    ///
    /// The two hydration flags mirror the proto fields and are independent on
    /// purpose, but a consumer that renders needs both: contracts give it the
    /// shape, present state gives it the values, and one without the other
    /// renders either empty forms or untyped blobs.
    pub async fn subscribe(
        &self,
        plugin_filters: Vec<String>,
        path_filters: Vec<String>,
        tag_filters: Vec<String>,
        include_initial_state: bool,
        include_schema: bool,
    ) -> Result<
        impl tokio_stream::Stream<Item = Result<StateUpdateMessage, GrpcClientError>>,
        GrpcClientError,
    > {
        let mut client = self.pool.state_sync_client(&self.default_address).await?;

        let mut request = Request::new(SubscribeRequest {
            plugin_ids: plugin_filters,
            path_patterns: path_filters,
            tags: tag_filters,
            include_initial_state,
            include_schema,
        });
        attach_ghostbridge_metadata(&mut request)?;

        let response = client
            .subscribe(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let stream = response.into_inner();

        Ok(tokio_stream::StreamExt::map(stream, |result| {
            result
                .map(|update| {
                    let change_type = change_type_name(update.change_type);
                    StateUpdateMessage {
                        change_id: update.change_id,
                        plugin_id: update.plugin_id,
                        object_path: update.object_path,
                        property_name: if update.member_name.is_empty() {
                            None
                        } else {
                            Some(update.member_name)
                        },
                        // Present on every frame that names a plugin, not just
                        // contract frames: it is what a consumer compares
                        // against the contract it is currently holding.
                        schema_hash: if update.schema_hash.is_empty() {
                            None
                        } else {
                            Some(update.schema_hash)
                        },
                        catalog_hash: update.catalog_hash,
                        old_value: update.old_value.as_ref().map(prost_value_to_simd),
                        new_value: update.new_value.as_ref().map(prost_value_to_simd),
                        event_id: update.event_id.to_string(),
                        event_hash: update.event_hash,
                        tags_touched: update.tags_touched,
                        actor_id: update.actor_id,
                        timestamp: update.timestamp.map(|ts| {
                            chrono::DateTime::from_timestamp(ts.seconds, ts.nanos.max(0) as u32)
                                .unwrap_or_default()
                                .to_rfc3339()
                        }),
                        change_type: change_type.to_string(),
                        frame_kind: frame_kind_name(update.frame_kind).to_string(),
                    }
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

        let mut request = Request::new(SubscribeEventsRequest {
            from_event_id: from_event_id.unwrap_or_default(),
            plugin_id: plugin_filters.first().cloned().unwrap_or_default(),
            tags: tag_filters,
        });
        attach_ghostbridge_metadata(&mut request)?;

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

/// State update message from subscription.
///
/// Every field the wire frame carries.
///
/// This is deliberately total rather than a useful subset: it is what
/// downstream transports (op-web's SSE bridge) re-serialize wholesale, so a
/// field omitted here is a field no browser subscriber can ever see. When the
/// proto message grows, grow this with it instead of letting consumers choose.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StateUpdateMessage {
    pub change_id: String,
    pub plugin_id: String,
    pub object_path: String,
    pub property_name: Option<String>,
    pub old_value: Option<simd_json::OwnedValue>,
    pub new_value: Option<simd_json::OwnedValue>,
    pub event_id: String,
    pub event_hash: String,
    pub tags_touched: Vec<String>,
    pub actor_id: String,
    /// RFC 3339, or `None` when the frame carried no timestamp.
    pub timestamp: Option<String>,
    /// Snake-case rendering of the proto `ChangeType`. Consumers that bridge
    /// this to another transport need it to tell a contract frame from a value
    /// frame; without it every frame looks like a property set.
    pub change_type: String,
    /// Snake-case rendering of the proto `StateFrameKind`: where in the stream
    /// this frame sits (hydration, live update, keepalive).
    pub frame_kind: String,
    /// Hash of the contract the frame's plugin is published under. On a
    /// `schema_migration` frame it identifies the contract in `new_value`; on
    /// every other frame it says which contract the value should be read
    /// against. `None` only when the frame names no plugin.
    pub schema_hash: Option<String>,
    /// Identity of the whole published catalog, on every frame including
    /// keepalives. A consumer that sees this change without having received the
    /// matching schema frame knows it missed one and must re-hydrate.
    pub catalog_hash: String,
}

/// Render a proto `ChangeType` discriminant as a stable snake-case name.
/// Unknown discriminants degrade to "unspecified" rather than failing the
/// stream — an older client must not drop frames a newer server adds.
fn change_type_name(value: i32) -> &'static str {
    match crate::proto::ChangeType::try_from(value) {
        Ok(crate::proto::ChangeType::PropertySet) => "property_set",
        Ok(crate::proto::ChangeType::PropertyDelete) => "property_delete",
        Ok(crate::proto::ChangeType::MethodCall) => "method_call",
        Ok(crate::proto::ChangeType::Signal) => "signal",
        Ok(crate::proto::ChangeType::ObjectAdded) => "object_added",
        Ok(crate::proto::ChangeType::ObjectRemoved) => "object_removed",
        Ok(crate::proto::ChangeType::SchemaMigration) => "schema_migration",
        Ok(crate::proto::ChangeType::Unspecified) | Err(_) => "unspecified",
    }
}

/// Render a proto `StateFrameKind` discriminant as a stable snake-case name.
fn frame_kind_name(value: i32) -> &'static str {
    match crate::proto::StateFrameKind::try_from(value) {
        Ok(crate::proto::StateFrameKind::InitialState) => "initial_state",
        Ok(crate::proto::StateFrameKind::Update) => "update",
        Ok(crate::proto::StateFrameKind::Heartbeat) => "heartbeat",
        Ok(crate::proto::StateFrameKind::Unspecified) | Err(_) => "unspecified",
    }
}

/// Chain event message from event stream.
///
/// Serialized wholesale by downstream transports for the same reason as
/// [`StateUpdateMessage`]: the hashes are what make the record verifiable, so
/// no hop gets to decide they are uninteresting.
#[derive(Debug, Clone, serde::Serialize)]
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

fn attach_ghostbridge_metadata<T>(request: &mut Request<T>) -> Result<(), GrpcClientError> {
    let (sled_ptr, _mmap) = op_identity::read_sled()
        .map_err(|e| GrpcClientError::RequestFailed(format!("Identity Sled unreadable: {e}")))?;
    let sled = unsafe { &*sled_ptr };

    if !sled.is_sled_valid() {
        return Err(GrpcClientError::RequestFailed(
            "Identity Sled is invalid; refusing unauthenticated gRPC call".to_string(),
        ));
    }

    let footprint = hex::encode(sled.hashed_footprint);
    let trace_id = sled.trace_id_hex();
    let metadata = request.metadata_mut();
    metadata.insert(
        "x-ghostbridge-footprint",
        MetadataValue::try_from(footprint).map_err(|e| {
            GrpcClientError::RequestFailed(format!("Invalid footprint metadata: {e}"))
        })?,
    );
    metadata.insert(
        "x-ghostbridge-trace-id",
        MetadataValue::try_from(trace_id)
            .map_err(|e| GrpcClientError::RequestFailed(format!("Invalid trace metadata: {e}")))?,
    );

    Ok(())
}

fn attach_supplied_ghostbridge_metadata<T>(
    request: &mut Request<T>,
    identity: &GhostbridgeCallMetadata,
) -> Result<(), GrpcClientError> {
    let metadata = request.metadata_mut();
    metadata.insert(
        "x-ghostbridge-footprint",
        MetadataValue::try_from(identity.footprint.as_str()).map_err(|e| {
            GrpcClientError::RequestFailed(format!("Invalid footprint metadata: {e}"))
        })?,
    );
    if let Some(trace_id) = identity.trace_id.as_deref() {
        metadata.insert(
            "x-ghostbridge-trace-id",
            MetadataValue::try_from(trace_id).map_err(|e| {
                GrpcClientError::RequestFailed(format!("Invalid trace metadata: {e}"))
            })?,
        );
    }
    if let Some(pubkey) = identity.wireguard_pubkey.as_deref() {
        metadata.insert(
            "x-wireguard-pubkey",
            MetadataValue::try_from(pubkey).map_err(|e| {
                GrpcClientError::RequestFailed(format!("Invalid WireGuard metadata: {e}"))
            })?,
        );
    }
    Ok(())
}

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

impl RemoteOperationClient {
    /// Join a Netmaker network through the Netmaker adapter gRPC service.
    pub async fn netmaker_join(&self, network: &str, token: &str) -> Result<bool, GrpcClientError> {
        let mut client = self.pool.netmaker_client(&self.default_address).await?;
        let response = client
            .join_network(Request::new(JoinNetworkRequest {
                network: network.to_string(),
                token: token.to_string(),
            }))
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;
        Ok(response.into_inner().success)
    }

    /// Leave a Netmaker network through the Netmaker adapter gRPC service.
    pub async fn netmaker_leave(&self, network: &str) -> Result<bool, GrpcClientError> {
        let mut client = self.pool.netmaker_client(&self.default_address).await?;
        let response = client
            .leave_network(Request::new(LeaveNetworkRequest {
                network: network.to_string(),
            }))
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;
        Ok(response.into_inner().success)
    }

    /// Restart a netclient-backed service through the adapter gRPC service.
    pub async fn netmaker_restart(&self, service: &str) -> Result<bool, GrpcClientError> {
        let mut client = self.pool.netmaker_client(&self.default_address).await?;
        let response = client
            .execute_command(Request::new(ExecuteCommandRequest {
                command: Some(ExecuteNetclientCommand::Restart(RestartServiceRequest {
                    service: service.to_string(),
                })),
            }))
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;
        Ok(response.into_inner().success)
    }

    pub async fn netclient_join(
        &self,
        network: &str,
        token: &str,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        let mut client = self.pool.netmaker_client(&self.default_address).await?;
        let response = client
            .execute_command(Request::new(ExecuteCommandRequest {
                command: Some(ExecuteNetclientCommand::Join(JoinNetworkRequest {
                    network: network.to_string(),
                    token: token.to_string(),
                })),
            }))
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;
        Ok(response.into_inner())
    }

    pub async fn netclient_leave(
        &self,
        network: &str,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        let mut client = self.pool.netmaker_client(&self.default_address).await?;
        let response = client
            .execute_command(Request::new(ExecuteCommandRequest {
                command: Some(ExecuteNetclientCommand::Leave(LeaveNetworkRequest {
                    network: network.to_string(),
                })),
            }))
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;
        Ok(response.into_inner())
    }

    pub async fn netclient_connect(
        &self,
        network: &str,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        let mut client = self.pool.netmaker_client(&self.default_address).await?;
        let response = client
            .execute_command(Request::new(ExecuteCommandRequest {
                command: Some(ExecuteNetclientCommand::Connect(ConnectRequest {
                    network: network.to_string(),
                })),
            }))
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;
        Ok(response.into_inner())
    }

    pub async fn netclient_disconnect(
        &self,
        network: &str,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        let mut client = self.pool.netmaker_client(&self.default_address).await?;
        let response = client
            .execute_command(Request::new(ExecuteCommandRequest {
                command: Some(ExecuteNetclientCommand::Disconnect(DisconnectRequest {
                    network: network.to_string(),
                })),
            }))
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;
        Ok(response.into_inner())
    }

    pub async fn netclient_list(&self) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::List(ListRequest {}))
            .await
    }

    pub async fn netclient_peers(&self) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Peers(PeersRequest {}))
            .await
    }

    pub async fn netclient_ping(
        &self,
        peer: &str,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Ping(PingRequest {
            peer: peer.to_string(),
        }))
        .await
    }

    pub async fn netclient_pull(&self) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Pull(PullRequest {}))
            .await
    }

    pub async fn netclient_push(&self) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Push(PushRequest {}))
            .await
    }

    pub async fn netclient_register(
        &self,
        instance: &str,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Register(RegisterRequest {
            instance: instance.to_string(),
        }))
        .await
    }

    pub async fn netclient_server(
        &self,
        subcommand: &str,
        args: Vec<String>,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Server(ServerRequest {
            subcommand: subcommand.to_string(),
            args,
        }))
        .await
    }

    pub async fn netclient_install(&self) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Install(InstallRequest {}))
            .await
    }

    pub async fn netclient_uninstall(&self) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Uninstall(UninstallRequest {}))
            .await
    }

    pub async fn netclient_use(
        &self,
        version: &str,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Use(UseRequest {
            version: version.to_string(),
        }))
        .await
    }

    pub async fn netclient_version(&self) -> Result<ExecuteCommandResponse, GrpcClientError> {
        self.netclient_noargs(ExecuteNetclientCommand::Version(VersionRequest {}))
            .await
    }

    pub async fn netclient_daemon(
        &self,
        _args: Vec<String>,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        let ok = self.netmaker_restart("netclient").await?;
        Ok(ExecuteCommandResponse {
            success: ok,
            exit_code: if ok { 0 } else { 1 },
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    async fn netclient_noargs(
        &self,
        command: ExecuteNetclientCommand,
    ) -> Result<ExecuteCommandResponse, GrpcClientError> {
        let mut client = self.pool.netmaker_client(&self.default_address).await?;
        let response = client
            .execute_command(Request::new(ExecuteCommandRequest {
                command: Some(command),
            }))
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;
        Ok(response.into_inner())
    }
}
