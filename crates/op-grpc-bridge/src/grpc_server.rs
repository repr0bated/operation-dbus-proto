//! gRPC Server - Implements the Operation gRPC services (shared-server topology)

use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::pin::Pin;
use std::sync::Arc;

use async_stream::stream;
use chrono::{DateTime, Utc};
use prost_types::{Struct as ProstStruct, Timestamp as ProstTimestamp, Value as ProstValue};
use simd_json::prelude::{ValueAsContainer, ValueAsScalar};
use tokio::sync::{broadcast, RwLock};
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::proto::{
    event_chain_service_server::EventChainService, ovsdb_mirror_server::OvsdbMirror,
    plugin_service_server::PluginService, runtime_mirror_server::RuntimeMirror,
    state_sync_server::StateSync, BatchMutateRequest, BatchMutateResponse, CallMethodRequest,
    CallMethodResponse, CapabilityMissing as ProtoCapabilityMissing, ChainEvent as ProtoChainEvent,
    ChangeType as ProtoChangeType, ConstraintFail as ProtoConstraintFail, CreateSnapshotRequest,
    CreateSnapshotResponse, Decision as ProtoDecision, DenyReason as ProtoDenyReason,
    ErrorCode as ProtoErrorCode, GetEventsRequest, GetEventsResponse, GetProofRequest,
    GetProofResponse, GetPropertyRequest, GetPropertyResponse, GetSchemaRequest, GetSchemaResponse,
    GetSnapshotRequest, GetSnapshotResponse, GetStateRequest, GetStateResponse, ListPluginsRequest,
    ListPluginsResponse, MerkleProofSibling, MutateRequest, MutateResponse,
    MutationError as ProtoMutationError, NumaNode as ProtoNumaNode,
    OperationType as ProtoOperationType, OvsdbBridge as ProtoOvsdbBridge, OvsdbDumpDbRequest,
    OvsdbDumpDbResponse, OvsdbEchoRequest, OvsdbEchoResponse, OvsdbGetBridgeStateRequest,
    OvsdbGetBridgeStateResponse, OvsdbGetSchemaRequest, OvsdbGetSchemaResponse,
    OvsdbInterface as ProtoOvsdbInterface, OvsdbListDbsRequest, OvsdbListDbsResponse,
    OvsdbMonitorRequest, OvsdbPort as ProtoOvsdbPort, OvsdbTransactRequest, OvsdbTransactResponse,
    OvsdbUpdate, PluginInfo, ProveTagImmutabilityRequest, ProveTagImmutabilityResponse,
    ReadOnlyViolation as ProtoReadOnlyViolation, RuntimeGetNumaTopologyRequest,
    RuntimeGetNumaTopologyResponse, RuntimeGetServiceRequest, RuntimeGetSystemInfoRequest,
    RuntimeGetSystemInfoResponse, RuntimeListInterfacesRequest, RuntimeListInterfacesResponse,
    RuntimeListServicesRequest, RuntimeListServicesResponse, RuntimeMetricUpdate,
    RuntimeNetworkInterface as ProtoRuntimeNetworkInterface,
    RuntimeServiceInfo as ProtoRuntimeServiceInfo, RuntimeStreamMetricsRequest, SetPropertyRequest,
    SetPropertyResponse, Signal, StateChange as ProtoStateChange, SubscribeEventsRequest,
    SubscribeRequest, SubscribeSignalsRequest, TagLock as ProtoTagLock, VerifyChainRequest,
    VerifyChainResponse,
};
use crate::sync_engine::{ChangeType, SyncEngine};
use op_state_store::{Decision, DenyReason, EventChain, MerkleProof, OperationType};
use zbus::zvariant::{Array as ZArray, OwnedValue as ZOwnedValue, Str as ZStr, Value as ZValue};
use zbus::{Connection, Proxy};

/// Plugin schema provider (source of truth)
pub trait PluginSchemaProvider: Send + Sync {
    fn list_plugins(&self) -> Vec<PluginInfo>;
    fn get_schema(&self, plugin_id: &str) -> Option<(String, String, String)>;
}

struct EmptyPluginProvider;

impl PluginSchemaProvider for EmptyPluginProvider {
    fn list_plugins(&self) -> Vec<PluginInfo> {
        Vec::new()
    }

    fn get_schema(&self, _plugin_id: &str) -> Option<(String, String, String)> {
        None
    }
}

// =============================================================================
// Registry State
// =============================================================================

/// In-memory component registry backing ComponentRegistry gRPC service.
/// Shared via Arc across all clones of OperationGrpcServer.
struct RegistryInner {
    /// component_id → ComponentInfo
    components: HashMap<String, crate::proto::registry::ComponentInfo>,
    /// component_id → lease_token
    leases: HashMap<String, String>,
    /// Broadcast channel for Watch stream
    watch_tx: broadcast::Sender<crate::proto::registry::RegistryEvent>,
}

impl RegistryInner {
    fn new() -> (
        Self,
        broadcast::Sender<crate::proto::registry::RegistryEvent>,
    ) {
        let (tx, _) = broadcast::channel(256);
        (
            Self {
                components: HashMap::new(),
                leases: HashMap::new(),
                watch_tx: tx.clone(),
            },
            tx,
        )
    }
}

// =============================================================================
// gRPC server implementation for operation services
// =============================================================================

#[derive(Clone)]
pub struct OperationGrpcServer {
    sync_engine: Arc<SyncEngine>,
    plugin_provider: Arc<dyn PluginSchemaProvider>,
    /// Broadcast channel for chain events
    chain_events: broadcast::Sender<ProtoChainEvent>,
    /// Component registry state (shared across clones)
    registry: Arc<RwLock<RegistryInner>>,
}

impl OperationGrpcServer {
    pub fn new(sync_engine: Arc<SyncEngine>) -> Self {
        let (chain_tx, _) = broadcast::channel(1024);
        let (registry, _) = RegistryInner::new();
        Self {
            sync_engine,
            plugin_provider: Arc::new(EmptyPluginProvider),
            chain_events: chain_tx,
            registry: Arc::new(RwLock::new(registry)),
        }
    }

    pub fn with_plugin_provider(
        sync_engine: Arc<SyncEngine>,
        plugin_provider: Arc<dyn PluginSchemaProvider>,
    ) -> Self {
        let (chain_tx, _) = broadcast::channel(1024);
        let (registry, _) = RegistryInner::new();
        Self {
            sync_engine,
            plugin_provider,
            chain_events: chain_tx,
            registry: Arc::new(RwLock::new(registry)),
        }
    }
}

/// Run gRPC server for all Operation services.
///
/// Includes:
///   - StateSync, PluginService, EventChainService, OvsdbMirror, RuntimeMirror
///   - gRPC server reflection (all protos in combined descriptor)
///   - gRPC health protocol (liveness for deploy verification and load balancers)
///
/// Adding a new domain service:
///   1. Add the generated server import below
///   2. Add `.add_service(...)` to the builder chain
///   3. Mark it serving via health_reporter
pub async fn run_grpc_server(
    addr: std::net::SocketAddr,
    sync_engine: Arc<SyncEngine>,
    plugin_provider: Option<Arc<dyn PluginSchemaProvider>>,
) -> Result<(), tonic::transport::Error> {
    use crate::proto::event_chain_service_server::EventChainServiceServer;
    use crate::proto::ovsdb_mirror_server::OvsdbMirrorServer;
    use crate::proto::plugin_service_server::PluginServiceServer;
    use crate::proto::registry::component_registry_server::ComponentRegistryServer;
    use crate::proto::runtime_mirror_server::RuntimeMirrorServer;
    use crate::proto::state_sync_server::StateSyncServer;

    let server = if let Some(provider) = plugin_provider {
        OperationGrpcServer::with_plugin_provider(sync_engine, provider)
    } else {
        OperationGrpcServer::new(sync_engine)
    };

    // Reflection — exposes combined FileDescriptorSet covering all domain protos.
    // Enables grpcurl discovery and drives MCP tool auto-registration in op-chat.
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("failed to build reflection service");

    // Health — standard gRPC health protocol for deploy verification and LB probes.
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<StateSyncServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<PluginServiceServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<EventChainServiceServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<OvsdbMirrorServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<RuntimeMirrorServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<ComponentRegistryServer<OperationGrpcServer>>()
        .await;

    info!(addr = %addr, "gRPC server starting (reflection + health enabled)");

    tonic::transport::Server::builder()
        .add_service(StateSyncServer::new(server.clone()))
        .add_service(PluginServiceServer::new(server.clone()))
        .add_service(EventChainServiceServer::new(server.clone()))
        .add_service(OvsdbMirrorServer::new(server.clone()))
        .add_service(RuntimeMirrorServer::new(server.clone()))
        .add_service(ComponentRegistryServer::new(server.clone()))
        .add_service(reflection)
        .add_service(health_service)
        .serve(addr)
        .await
}

// =============================================================================
// StateSync Service
// =============================================================================

#[tonic::async_trait]
impl StateSync for OperationGrpcServer {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<ProtoStateChange, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        info!("gRPC Subscribe: plugins={:?}", req.plugin_ids);

        let mut rx = self.sync_engine.change_sender().subscribe();
        let plugin_filters = req.plugin_ids;
        let path_filters = req.path_patterns;
        let tag_filters = req.tags;

        let stream = stream! {
            loop {
                match rx.recv().await {
                    Ok(update) => {
                        let matches_plugin = plugin_filters.is_empty()
                            || plugin_filters.contains(&update.plugin_id);
                        let matches_path = path_filters.is_empty()
                            || path_filters.iter().any(|p| update.object_path.starts_with(p));
                        let matches_tag = tag_filters.is_empty()
                            || update.tags_touched.iter().any(|t| tag_filters.contains(t));

                        if matches_plugin && matches_path && matches_tag {
                            yield Ok(proto_state_change(&update));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Subscriber lagged, missed {} updates", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }

    async fn mutate(
        &self,
        request: Request<MutateRequest>,
    ) -> Result<Response<MutateResponse>, Status> {
        let req = request.into_inner();
        let value = prost_value_to_simd(&req.value.unwrap_or_else(|| ProstValue::from(0)));
        let change_type = match req.operation {
            x if x == ProtoOperationType::SetProperty as i32 => ChangeType::PropertySet,
            x if x == ProtoOperationType::CallMethod as i32 => ChangeType::MethodCall,
            x if x == ProtoOperationType::ApplyPatch as i32 => ChangeType::ObjectAdded,
            _ => ChangeType::PropertySet,
        };

        let result = self
            .sync_engine
            .process_grpc_mutation(
                req.plugin_id.clone(),
                req.object_path.clone(),
                change_type,
                if req.member_name.is_empty() {
                    None
                } else {
                    Some(req.member_name.clone())
                },
                value,
                req.actor_id.clone(),
                if req.capability_id.is_empty() {
                    None
                } else {
                    Some(req.capability_id.clone())
                },
            )
            .await;

        match result {
            Ok(ok) => Ok(Response::new(MutateResponse {
                success: ok.success,
                event_id: ok.event_id,
                event_hash: ok.event_hash,
                result: ok.result.map(|v| simd_to_prost_value(&v)),
                error: None,
                effective_hash: String::new(),
            })),
            Err(e) => Ok(Response::new(MutateResponse {
                success: false,
                event_id: 0,
                event_hash: String::new(),
                result: None,
                error: Some(ProtoMutationError {
                    code: ProtoErrorCode::Internal as i32,
                    message: e.to_string(),
                    deny_reason: None,
                }),
                effective_hash: String::new(),
            })),
        }
    }

    async fn get_state(
        &self,
        request: Request<GetStateRequest>,
    ) -> Result<Response<GetStateResponse>, Status> {
        let req = request.into_inner();
        let state = self.sync_engine.get_state(&req.plugin_id).await;

        let state_struct = state
            .map(|v| simd_to_prost_struct(&v))
            .unwrap_or_else(ProstStruct::default);

        Ok(Response::new(GetStateResponse {
            state: Some(state_struct),
            effective_hash: String::new(),
            at_event_id: 0,
        }))
    }

    async fn batch_mutate(
        &self,
        request: Request<BatchMutateRequest>,
    ) -> Result<Response<BatchMutateResponse>, Status> {
        let req = request.into_inner();
        let mut results = Vec::new();
        let mut failed_index = -1;

        for (idx, m) in req.mutations.into_iter().enumerate() {
            let mut_req = Request::new(m);
            let resp = self.mutate(mut_req).await?.into_inner();
            if !resp.success && failed_index < 0 && req.atomic {
                failed_index = idx as i32;
                break;
            }
            results.push(resp);
        }

        Ok(Response::new(BatchMutateResponse {
            success: failed_index < 0,
            results,
            failed_index,
        }))
    }
}

// =============================================================================
// PluginService
// =============================================================================

#[tonic::async_trait]
impl PluginService for OperationGrpcServer {
    type SubscribeSignalsStream = Pin<Box<dyn Stream<Item = Result<Signal, Status>> + Send>>;

    async fn list_plugins(
        &self,
        _request: Request<ListPluginsRequest>,
    ) -> Result<Response<ListPluginsResponse>, Status> {
        Ok(Response::new(ListPluginsResponse {
            plugins: self.plugin_provider.list_plugins(),
        }))
    }

    async fn get_schema(
        &self,
        request: Request<GetSchemaRequest>,
    ) -> Result<Response<GetSchemaResponse>, Status> {
        let req = request.into_inner();
        if let Some((schema_json, dialect, version)) =
            self.plugin_provider.get_schema(&req.plugin_id)
        {
            Ok(Response::new(GetSchemaResponse {
                schema_json,
                dialect,
                version,
            }))
        } else {
            Ok(Response::new(GetSchemaResponse {
                schema_json: String::new(),
                dialect: String::new(),
                version: String::new(),
            }))
        }
    }

    async fn call_method(
        &self,
        request: Request<CallMethodRequest>,
    ) -> Result<Response<CallMethodResponse>, Status> {
        let req = request.into_inner();
        let args: Vec<simd_json::OwnedValue> = req
            .arguments
            .into_iter()
            .map(|v| prost_value_to_simd(&v))
            .collect();

        let result = self
            .sync_engine
            .call_dbus_method(
                &format!("org.opdbus.{}.v1", req.plugin_id),
                &req.object_path,
                &req.interface_name,
                &req.method_name,
                args,
                &req.actor_id,
                &if req.capability_id.is_empty() {
                    None
                } else {
                    Some(req.capability_id.clone())
                },
            )
            .await;

        match result {
            Ok(val) => Ok(Response::new(CallMethodResponse {
                success: true,
                result: Some(simd_to_prost_value(&val)),
                event_id: 0,
                event_hash: String::new(),
                error: None,
            })),
            Err(e) => Ok(Response::new(CallMethodResponse {
                success: false,
                result: None,
                event_id: 0,
                event_hash: String::new(),
                error: Some(ProtoMutationError {
                    code: ProtoErrorCode::Internal as i32,
                    message: e.to_string(),
                    deny_reason: None,
                }),
            })),
        }
    }

    async fn get_property(
        &self,
        request: Request<GetPropertyRequest>,
    ) -> Result<Response<GetPropertyResponse>, Status> {
        let req = request.into_inner();
        let connection = Connection::system()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let proxy = zbus::fdo::PropertiesProxy::builder(&connection)
            .destination(format!("org.opdbus.{}.v1", req.plugin_id))
            .map_err(|e| Status::internal(e.to_string()))?
            .path(req.object_path.as_str())
            .map_err(|e| Status::internal(e.to_string()))?
            .build()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let iface = zbus::names::InterfaceName::try_from(req.interface_name.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let val: ZOwnedValue = proxy
            .get(iface, req.property_name.as_str())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let json =
            simd_json::serde::to_owned_value(&val).map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetPropertyResponse {
            value: Some(simd_to_prost_value(&json)),
            read_only: false,
        }))
    }

    async fn set_property(
        &self,
        request: Request<SetPropertyRequest>,
    ) -> Result<Response<SetPropertyResponse>, Status> {
        let req = request.into_inner();
        let connection = Connection::system()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let proxy = zbus::fdo::PropertiesProxy::builder(&connection)
            .destination(format!("org.opdbus.{}.v1", req.plugin_id))
            .map_err(|e| Status::internal(e.to_string()))?
            .path(req.object_path.as_str())
            .map_err(|e| Status::internal(e.to_string()))?
            .build()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let iface = zbus::names::InterfaceName::try_from(req.interface_name.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let value = prost_value_to_simd(&req.value.unwrap_or_else(|| ProstValue::from(0)));
        let zval =
            simd_json_to_zvariant(&value).map_err(|e| Status::invalid_argument(e.to_string()))?;

        proxy
            .set(iface, req.property_name.as_str(), &zval)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(SetPropertyResponse {
            success: true,
            event_id: 0,
            event_hash: String::new(),
            error: None,
        }))
    }

    async fn subscribe_signals(
        &self,
        _request: Request<SubscribeSignalsRequest>,
    ) -> Result<Response<Self::SubscribeSignalsStream>, Status> {
        let stream = tokio_stream::empty::<Result<Signal, Status>>();
        Ok(Response::new(Box::pin(stream)))
    }
}

// =============================================================================
// EventChainService
// =============================================================================

#[tonic::async_trait]
impl EventChainService for OperationGrpcServer {
    type SubscribeEventsStream =
        Pin<Box<dyn Stream<Item = Result<ProtoChainEvent, Status>> + Send>>;

    async fn get_events(
        &self,
        request: Request<GetEventsRequest>,
    ) -> Result<Response<GetEventsResponse>, Status> {
        let req = request.into_inner();
        let chain = self.sync_engine.event_chain();
        let chain = chain.read().await;

        let mut events: Vec<ProtoChainEvent> = chain
            .events()
            .iter()
            .filter(|e| req.from_event_id == 0 || e.event_id >= req.from_event_id)
            .filter(|e| req.to_event_id == 0 || e.event_id <= req.to_event_id)
            .filter(|e| req.plugin_id.is_empty() || e.plugin_id == req.plugin_id)
            .filter(|e| req.tags.is_empty() || e.tags_touched.iter().any(|t| req.tags.contains(t)))
            .filter(|e| match req.decision_filter {
                x if x == ProtoDecision::Allow as i32 => e.decision == Decision::Allow,
                x if x == ProtoDecision::Deny as i32 => e.decision == Decision::Deny,
                _ => true,
            })
            .take(if req.limit == 0 {
                usize::MAX
            } else {
                req.limit as usize
            })
            .map(proto_chain_event)
            .collect();

        let has_more = req.limit > 0 && (events.len() as u32) == req.limit;
        Ok(Response::new(GetEventsResponse { events, has_more }))
    }

    async fn subscribe_events(
        &self,
        request: Request<SubscribeEventsRequest>,
    ) -> Result<Response<Self::SubscribeEventsStream>, Status> {
        let req = request.into_inner();
        let mut rx = self.chain_events.subscribe();
        let plugin_filter = req.plugin_id;
        let tag_filters = req.tags;

        let stream = stream! {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let matches_plugin = plugin_filter.is_empty() || event.plugin_id == plugin_filter;
                        let matches_tag = tag_filters.is_empty() || event.tags_touched.iter().any(|t| tag_filters.contains(t));
                        if matches_plugin && matches_tag {
                            yield Ok(event);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }

    async fn verify_chain(
        &self,
        _request: Request<VerifyChainRequest>,
    ) -> Result<Response<VerifyChainResponse>, Status> {
        let chain = self.sync_engine.event_chain();
        let chain = chain.read().await;
        let result = chain.verify_chain();
        Ok(Response::new(VerifyChainResponse {
            valid: result.valid,
            events_verified: result.events_verified as u64,
            batches_verified: result.batches_verified as u64,
            errors: result.errors,
        }))
    }

    async fn get_proof(
        &self,
        request: Request<GetProofRequest>,
    ) -> Result<Response<GetProofResponse>, Status> {
        let req = request.into_inner();
        let chain = self.sync_engine.event_chain();
        let chain = chain.read().await;
        let proof: Option<MerkleProof> =
            op_state_store::EventBatch::generate_proof(chain.events(), req.event_id);

        if let Some(proof) = proof {
            let siblings = proof
                .siblings
                .into_iter()
                .map(|(hash, is_right)| MerkleProofSibling { hash, is_right })
                .collect();
            Ok(Response::new(GetProofResponse {
                event_hash: proof.event_hash,
                siblings,
                root: proof.root,
                batch_first_event_id: 0,
                batch_last_event_id: 0,
            }))
        } else {
            Err(Status::not_found("proof not found"))
        }
    }

    async fn prove_tag_immutability(
        &self,
        request: Request<ProveTagImmutabilityRequest>,
    ) -> Result<Response<ProveTagImmutabilityResponse>, Status> {
        let req = request.into_inner();
        let chain = self.sync_engine.event_chain();
        let chain = chain.read().await;
        let proof = chain.prove_tag_immutability(&req.tag);
        Ok(Response::new(ProveTagImmutabilityResponse {
            tag: proof.tag,
            is_immutable: proof.is_immutable,
            violation_event_ids: proof.violations,
            total_events_checked: proof.total_events_checked as u64,
        }))
    }

    async fn get_snapshot(
        &self,
        request: Request<GetSnapshotRequest>,
    ) -> Result<Response<GetSnapshotResponse>, Status> {
        let req = request.into_inner();
        let chain = self.sync_engine.event_chain();
        let chain = chain.read().await;
        if let Some(snapshot) = chain.get_snapshot(&req.snapshot_id) {
            Ok(Response::new(GetSnapshotResponse {
                snapshot: Some(proto_snapshot(snapshot)),
            }))
        } else {
            Err(Status::not_found("snapshot not found"))
        }
    }

    async fn create_snapshot(
        &self,
        request: Request<CreateSnapshotRequest>,
    ) -> Result<Response<CreateSnapshotResponse>, Status> {
        let req = request.into_inner();
        let state = self
            .sync_engine
            .get_state(&req.plugin_id)
            .await
            .unwrap_or_else(|| simd_json::json!({}));
        let chain = self.sync_engine.event_chain();
        let mut chain = chain.write().await;
        let snapshot = chain.create_snapshot(req.plugin_id, "1.0.0".to_string(), state);
        Ok(Response::new(CreateSnapshotResponse {
            snapshot: Some(proto_snapshot(snapshot)),
        }))
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn proto_state_change(change: &crate::sync_engine::StateChange) -> ProtoStateChange {
    ProtoStateChange {
        change_id: change.change_id.clone(),
        event_id: change.event_id,
        plugin_id: change.plugin_id.clone(),
        object_path: change.object_path.clone(),
        change_type: proto_change_type(change.change_type) as i32,
        member_name: change.member_name.clone().unwrap_or_default(),
        old_value: change.old_value.as_ref().map(simd_to_prost_value),
        new_value: Some(simd_to_prost_value(&change.new_value)),
        tags_touched: change.tags_touched.clone(),
        event_hash: change.event_hash.clone(),
        timestamp: Some(proto_timestamp(change.timestamp)),
        actor_id: change.actor_id.clone(),
    }
}

fn proto_change_type(change_type: ChangeType) -> ProtoChangeType {
    match change_type {
        ChangeType::PropertySet => ProtoChangeType::PropertySet,
        ChangeType::PropertyDelete => ProtoChangeType::PropertyDelete,
        ChangeType::MethodCall => ProtoChangeType::MethodCall,
        ChangeType::Signal => ProtoChangeType::Signal,
        ChangeType::ObjectAdded => ProtoChangeType::ObjectAdded,
        ChangeType::ObjectRemoved => ProtoChangeType::ObjectRemoved,
        ChangeType::SchemaMigration => ProtoChangeType::SchemaMigration,
    }
}

fn proto_chain_event(event: &op_state_store::ChainEvent) -> ProtoChainEvent {
    ProtoChainEvent {
        event_id: event.event_id,
        prev_hash: event.prev_hash.clone(),
        event_hash: event.event_hash.clone(),
        timestamp: Some(proto_timestamp(event.timestamp)),
        actor_id: event.actor_id.clone(),
        capability_id: event.capability_id.clone().unwrap_or_default(),
        plugin_id: event.plugin_id.clone(),
        schema_version: event.schema_version.clone(),
        operation_type: format!("{:?}", event.op),
        target: event.target.clone(),
        tags_touched: event.tags_touched.clone(),
        decision: match event.decision {
            Decision::Allow => ProtoDecision::Allow as i32,
            Decision::Deny => ProtoDecision::Deny as i32,
        },
        deny_reason: event.deny_reason.as_ref().map(proto_deny_reason),
        input_patch_hash: event.input_patch_hash.clone(),
        result_effective_hash: event.result_effective_hash.clone().unwrap_or_default(),
    }
}

fn proto_deny_reason(reason: &DenyReason) -> ProtoDenyReason {
    match reason {
        DenyReason::TagLock { tag, wrapper_id } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::TagLock(ProtoTagLock {
                tag: tag.clone(),
                wrapper_id: wrapper_id.clone(),
            })),
        },
        DenyReason::ConstraintFail {
            constraint,
            message,
        } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::ConstraintFail(
                ProtoConstraintFail {
                    constraint: constraint.clone(),
                    message: message.clone(),
                },
            )),
        },
        DenyReason::CapabilityMissing { capability } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::CapabilityMissing(
                ProtoCapabilityMissing {
                    capability: capability.clone(),
                },
            )),
        },
        DenyReason::ReadOnlyViolation { field } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::ReadOnlyViolation(
                ProtoReadOnlyViolation {
                    field: field.clone(),
                },
            )),
        },
        DenyReason::SchemaValidation { errors } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::ConstraintFail(
                ProtoConstraintFail {
                    constraint: "schema_validation".to_string(),
                    message: errors.join("; "),
                },
            )),
        },
        DenyReason::Custom { reason } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::ConstraintFail(
                ProtoConstraintFail {
                    constraint: "custom".to_string(),
                    message: reason.clone(),
                },
            )),
        },
    }
}

fn proto_snapshot(snapshot: &op_state_store::StateSnapshot) -> crate::proto::Snapshot {
    crate::proto::Snapshot {
        snapshot_id: snapshot.snapshot_id.clone(),
        at_event_id: snapshot.at_event_id,
        plugin_id: snapshot.plugin_id.clone(),
        schema_version: snapshot.schema_version.clone(),
        stub_hash: snapshot.stub_hash.clone(),
        immutable_wrappers_hash: snapshot.immutable_wrappers_hash.clone(),
        tunable_patch_hash: snapshot.tunable_patch_hash.clone(),
        effective_hash: snapshot.effective_hash.clone(),
        timestamp: Some(proto_timestamp(snapshot.timestamp)),
        state: Some(simd_to_prost_struct(&snapshot.state)),
    }
}

fn proto_timestamp(ts: DateTime<Utc>) -> ProstTimestamp {
    ProstTimestamp {
        seconds: ts.timestamp(),
        nanos: ts.timestamp_subsec_nanos() as i32,
    }
}

fn simd_to_prost_struct(value: &simd_json::OwnedValue) -> ProstStruct {
    match value.as_object() {
        Some(map) => {
            let fields = map
                .iter()
                .map(|(k, v)| (k.to_string(), simd_to_prost_value(v)))
                .collect();
            ProstStruct { fields }
        }
        None => ProstStruct {
            fields: BTreeMap::new(),
        },
    }
}

fn simd_to_prost_value(value: &simd_json::OwnedValue) -> ProstValue {
    use prost_types::value::Kind;
    if value.as_null().is_some() {
        return ProstValue {
            kind: Some(Kind::NullValue(0)),
        };
    }
    if let Some(b) = value.as_bool() {
        return ProstValue {
            kind: Some(Kind::BoolValue(b)),
        };
    }
    if let Some(n) = value.as_f64() {
        return ProstValue {
            kind: Some(Kind::NumberValue(n)),
        };
    }
    if let Some(s) = value.as_str() {
        return ProstValue {
            kind: Some(Kind::StringValue(s.to_string())),
        };
    }
    if let Some(arr) = value.as_array() {
        let vals = arr.iter().map(simd_to_prost_value).collect();
        return ProstValue {
            kind: Some(Kind::ListValue(prost_types::ListValue { values: vals })),
        };
    }
    if let Some(obj) = value.as_object() {
        let fields = obj
            .iter()
            .map(|(k, v)| (k.to_string(), simd_to_prost_value(v)))
            .collect();
        return ProstValue {
            kind: Some(Kind::StructValue(ProstStruct { fields })),
        };
    }
    ProstValue {
        kind: Some(Kind::NullValue(0)),
    }
}

fn prost_value_to_simd(value: &ProstValue) -> simd_json::OwnedValue {
    use prost_types::value::Kind;
    match &value.kind {
        None => simd_json::json!(null),
        Some(Kind::NullValue(_)) => simd_json::json!(null),
        Some(Kind::BoolValue(b)) => simd_json::json!(*b),
        Some(Kind::NumberValue(n)) => simd_json::json!(*n),
        Some(Kind::StringValue(s)) => simd_json::json!(s),
        Some(Kind::StructValue(s)) => {
            let mut map = simd_json::value::owned::Object::new();
            for (k, v) in &s.fields {
                map.insert(k.clone(), prost_value_to_simd(v));
            }
            simd_json::OwnedValue::Object(Box::new(map))
        }
        Some(Kind::ListValue(l)) => {
            let arr = l.values.iter().map(prost_value_to_simd).collect::<Vec<_>>();
            simd_json::OwnedValue::from(arr)
        }
    }
}

fn simd_json_to_zvariant(value: &simd_json::OwnedValue) -> Result<ZOwnedValue, anyhow::Error> {
    if let Some(obj) = value.as_object() {
        if let (Some(sig_val), Some(inner)) = (obj.get("sig"), obj.get("value")) {
            if let Some(sig) = sig_val.as_str() {
                return zvariant_from_sig(sig, inner);
            }
        }
    }

    if let Some(s) = value.as_str() {
        return Ok(ZOwnedValue::from(ZStr::from(s)));
    }
    if let Some(b) = value.as_bool() {
        return Ok(ZOwnedValue::from(b));
    }
    if let Some(i) = value.as_i64() {
        return Ok(ZOwnedValue::from(i));
    }
    if let Some(u) = value.as_u64() {
        return Ok(ZOwnedValue::from(u));
    }
    if let Some(f) = value.as_f64() {
        return Ok(ZOwnedValue::from(f));
    }

    Err(anyhow::anyhow!(
        "Unsupported argument type; use tagged {{sig,value}} or primitives"
    ))
}

fn zvariant_from_sig(
    sig: &str,
    value: &simd_json::OwnedValue,
) -> Result<ZOwnedValue, anyhow::Error> {
    match sig {
        "s" => value
            .as_str()
            .map(|v| ZOwnedValue::from(ZStr::from(v)))
            .ok_or_else(|| anyhow::anyhow!("Expected string for sig 's'")),
        "b" => value
            .as_bool()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected bool for sig 'b'")),
        "i" => value
            .as_i64()
            .map(|v| ZOwnedValue::from(v as i32))
            .ok_or_else(|| anyhow::anyhow!("Expected i32 for sig 'i'")),
        "u" => value
            .as_u64()
            .map(|v| ZOwnedValue::from(v as u32))
            .ok_or_else(|| anyhow::anyhow!("Expected u32 for sig 'u'")),
        "x" => value
            .as_i64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected i64 for sig 'x'")),
        "t" => value
            .as_u64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected u64 for sig 't'")),
        "d" => value
            .as_f64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected f64 for sig 'd'")),
        "ay" => {
            let arr = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected array for sig 'ay'"))?;
            let bytes: Result<Vec<u8>, anyhow::Error> = arr
                .iter()
                .map(|v| {
                    v.as_u64()
                        .map(|n| n as u8)
                        .ok_or_else(|| anyhow::anyhow!("Expected u8 in ay array"))
                })
                .collect();
            ZOwnedValue::try_from(ZValue::Array(ZArray::from(bytes?)))
                .map_err(|e| anyhow::anyhow!("Array conversion error: {}", e))
        }
        _ => Err(anyhow::anyhow!("Unsupported signature '{}'", sig)),
    }
}

// =============================================================================
// OvsdbMirror Service — RFC 7047 gRPC bridge
// =============================================================================

#[tonic::async_trait]
impl OvsdbMirror for OperationGrpcServer {
    type MonitorStream = Pin<Box<dyn Stream<Item = Result<OvsdbUpdate, Status>> + Send>>;

    async fn list_dbs(
        &self,
        _request: Request<OvsdbListDbsRequest>,
    ) -> Result<Response<OvsdbListDbsResponse>, Status> {
        let result = self.ovsdb_call("list_dbs", "[]").await?;
        let dbs: Vec<String> = serde_json::from_str(&result)
            .map_err(|e| Status::internal(format!("Parse error: {}", e)))?;
        Ok(Response::new(OvsdbListDbsResponse { databases: dbs }))
    }

    async fn get_schema(
        &self,
        request: Request<OvsdbGetSchemaRequest>,
    ) -> Result<Response<OvsdbGetSchemaResponse>, Status> {
        let db = &request.get_ref().database;
        let db_arg = if db.is_empty() { "Open_vSwitch" } else { db };
        let result = self
            .ovsdb_call("get_schema", &format!("[\"{}\"]", db_arg))
            .await?;
        Ok(Response::new(OvsdbGetSchemaResponse {
            schema_json: result,
            name: db_arg.to_string(),
            version: String::new(),
        }))
    }

    async fn transact(
        &self,
        request: Request<OvsdbTransactRequest>,
    ) -> Result<Response<OvsdbTransactResponse>, Status> {
        let req = request.get_ref();
        let db = if req.database.is_empty() {
            "Open_vSwitch"
        } else {
            &req.database
        };
        let ops = &req.operations_json;
        let call_arg = format!("[\"{}\", {}]", db, ops);
        match self.ovsdb_call("transact", &call_arg).await {
            Ok(result) => Ok(Response::new(OvsdbTransactResponse {
                success: true,
                results_json: result,
                event_id: 0,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(OvsdbTransactResponse {
                success: false,
                results_json: String::new(),
                event_id: 0,
                error: e.message().to_string(),
            })),
        }
    }

    async fn monitor(
        &self,
        _request: Request<OvsdbMonitorRequest>,
    ) -> Result<Response<Self::MonitorStream>, Status> {
        // TODO: Wire to OVSDB monitor via D-Bus mirror's monitor channel
        let stream = stream! {
            // Placeholder — will connect to ovsdb monitor_db broadcast
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            yield Err(Status::cancelled("Monitor stream ended"));
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn echo(
        &self,
        request: Request<OvsdbEchoRequest>,
    ) -> Result<Response<OvsdbEchoResponse>, Status> {
        Ok(Response::new(OvsdbEchoResponse {
            payload: request.into_inner().payload,
        }))
    }

    async fn dump_db(
        &self,
        request: Request<OvsdbDumpDbRequest>,
    ) -> Result<Response<OvsdbDumpDbResponse>, Status> {
        let db = &request.get_ref().database;
        let db_arg = if db.is_empty() { "Open_vSwitch" } else { db };
        let result = self
            .ovsdb_call("dump", &format!("[\"{}\"]", db_arg))
            .await?;
        Ok(Response::new(OvsdbDumpDbResponse { dump_json: result }))
    }

    async fn get_bridge_state(
        &self,
        request: Request<OvsdbGetBridgeStateRequest>,
    ) -> Result<Response<OvsdbGetBridgeStateResponse>, Status> {
        let filter = &request.get_ref().bridge_name;

        // Query via D-Bus mirror's OvsdbV1 interface
        let dump = self.ovsdb_call("dump", "[\"Open_vSwitch\"]").await?;

        // Parse and build bridge hierarchy
        let bridges = self
            .parse_bridge_hierarchy(&dump, filter)
            .map_err(|e| Status::internal(format!("Parse error: {}", e)))?;

        Ok(Response::new(OvsdbGetBridgeStateResponse { bridges }))
    }
}

impl OperationGrpcServer {
    /// Call an OVSDB method via the D-Bus mirror interface
    async fn ovsdb_call(&self, method: &str, args: &str) -> Result<String, Status> {
        let conn = self
            .sync_engine
            .dbus_connection()
            .await
            .map_err(|e| Status::unavailable(format!("D-Bus not available: {}", e)))?;

        let proxy = Proxy::new(
            &conn,
            "org.opdbus.v1",
            "/org/opdbus/v1/ovsdb",
            "org.opdbus.OvsdbV1",
        )
        .await
        .map_err(|e| Status::internal(format!("Proxy error: {}", e)))?;

        let result: String = proxy
            .call(method, &(args.to_string(),))
            .await
            .map_err(|e| Status::internal(format!("OVSDB call '{}' failed: {}", method, e)))?;

        Ok(result)
    }

    /// Parse OVSDB dump into Bridge→Port→Interface hierarchy
    fn parse_bridge_hierarchy(
        &self,
        dump_json: &str,
        filter: &str,
    ) -> Result<Vec<ProtoOvsdbBridge>, anyhow::Error> {
        let dump: serde_json::Value = serde_json::from_str(dump_json)?;
        let mut bridges = Vec::new();

        // Get Bridge table rows
        let empty_map = serde_json::Map::new();
        let bridge_rows = dump
            .get("Bridge")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty_map);

        for (_uuid, row) in bridge_rows {
            let name = row
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !filter.is_empty() && name != filter {
                continue;
            }

            let datapath_type = row
                .get("datapath_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let fail_mode = row
                .get("fail_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let stp_enable = row
                .get("stp_enable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let mcast_snooping_enable = row
                .get("mcast_snooping_enable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Parse ports (OVSDB set format: ["set", [...]] or ["uuid", "..."])
            let port_uuids = self.extract_uuid_set(row.get("ports"));

            let mut ports = Vec::new();
            let empty_port_map = serde_json::Map::new();
            let port_rows = dump
                .get("Port")
                .and_then(|v| v.as_object())
                .unwrap_or(&empty_port_map);

            for port_uuid in &port_uuids {
                if let Some(port_row) = port_rows.get(port_uuid) {
                    let port_name = port_row
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tag = port_row.get("tag").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                    // Parse interfaces
                    let iface_uuids = self.extract_uuid_set(port_row.get("interfaces"));
                    let empty_iface_map = serde_json::Map::new();
                    let iface_rows = dump
                        .get("Interface")
                        .and_then(|v| v.as_object())
                        .unwrap_or(&empty_iface_map);

                    let mut interfaces = Vec::new();
                    for iface_uuid in &iface_uuids {
                        if let Some(iface_row) = iface_rows.get(iface_uuid) {
                            interfaces.push(ProtoOvsdbInterface {
                                name: iface_row
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                r#type: iface_row
                                    .get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                mac_in_use: iface_row
                                    .get("mac_in_use")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                mac: iface_row
                                    .get("mac")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                admin_state: iface_row
                                    .get("admin_state")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                link_state: iface_row
                                    .get("link_state")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                options: self.extract_map(iface_row.get("options")),
                            });
                        }
                    }

                    ports.push(ProtoOvsdbPort {
                        name: port_name,
                        tag,
                        trunks: vec![],
                        vlan_mode: port_row
                            .get("vlan_mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        bond_mode: port_row
                            .get("bond_mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        interfaces,
                    });
                }
            }

            bridges.push(ProtoOvsdbBridge {
                name,
                datapath_type,
                fail_mode,
                stp_enable,
                mcast_snooping_enable,
                other_config: self.extract_map(row.get("other_config")),
                ports,
            });
        }

        Ok(bridges)
    }

    /// Extract UUID set from OVSDB value (["set", [...]] or ["uuid", "..."])
    fn extract_uuid_set(&self, value: Option<&serde_json::Value>) -> Vec<String> {
        let Some(v) = value else {
            return vec![];
        };
        if let Some(arr) = v.as_array() {
            if arr.len() == 2 {
                if arr[0].as_str() == Some("uuid") {
                    return vec![arr[1].as_str().unwrap_or("").to_string()];
                }
                if arr[0].as_str() == Some("set") {
                    if let Some(items) = arr[1].as_array() {
                        return items
                            .iter()
                            .filter_map(|item| {
                                if let Some(inner) = item.as_array() {
                                    if inner.len() == 2 && inner[0].as_str() == Some("uuid") {
                                        return inner[1].as_str().map(|s| s.to_string());
                                    }
                                }
                                None
                            })
                            .collect();
                    }
                }
            }
        }
        vec![]
    }

    /// Extract OVSDB map (["map", [[k,v], ...]]) to HashMap
    fn extract_map(
        &self,
        value: Option<&serde_json::Value>,
    ) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        let Some(v) = value else {
            return map;
        };
        if let Some(arr) = v.as_array() {
            if arr.len() == 2 && arr[0].as_str() == Some("map") {
                if let Some(pairs) = arr[1].as_array() {
                    for pair in pairs {
                        if let Some(kv) = pair.as_array() {
                            if kv.len() == 2 {
                                let k = kv[0].as_str().unwrap_or("").to_string();
                                let v = kv[1].as_str().unwrap_or("").to_string();
                                map.insert(k, v);
                            }
                        }
                    }
                }
            }
        }
        map
    }
}

// =============================================================================
// RuntimeMirror Service — Live operational state
// =============================================================================

#[tonic::async_trait]
impl RuntimeMirror for OperationGrpcServer {
    type StreamMetricsStream =
        Pin<Box<dyn Stream<Item = Result<RuntimeMetricUpdate, Status>> + Send>>;

    async fn get_system_info(
        &self,
        _request: Request<RuntimeGetSystemInfoRequest>,
    ) -> Result<Response<RuntimeGetSystemInfoResponse>, Status> {
        let hostname = tokio::fs::read_to_string("/etc/hostname")
            .await
            .unwrap_or_default()
            .trim()
            .to_string();
        let kernel_version = tokio::fs::read_to_string("/proc/version")
            .await
            .unwrap_or_default()
            .split_whitespace()
            .nth(2)
            .unwrap_or("")
            .to_string();
        let uptime_str = tokio::fs::read_to_string("/proc/uptime")
            .await
            .unwrap_or_default();
        let uptime_seconds = uptime_str
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;

        let meminfo = tokio::fs::read_to_string("/proc/meminfo")
            .await
            .unwrap_or_default();
        let mem_total = Self::parse_meminfo_kb(&meminfo, "MemTotal") * 1024;
        let mem_available = Self::parse_meminfo_kb(&meminfo, "MemAvailable") * 1024;
        let mem_used = mem_total.saturating_sub(mem_available);

        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        let arch = std::env::consts::ARCH.to_string();

        // Detect init system
        let init_system = if std::path::Path::new("/run/dinitctl").exists() {
            "dinit"
        } else {
            "systemd"
        }
        .to_string();

        Ok(Response::new(RuntimeGetSystemInfoResponse {
            hostname,
            kernel_version,
            uptime_seconds,
            boot_timestamp: 0,
            cpu_count,
            memory_total_bytes: mem_total,
            memory_available_bytes: mem_available,
            memory_used_bytes: mem_used,
            init_system,
            arch,
            queried_at: Some(ProstTimestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
        }))
    }

    async fn list_services(
        &self,
        _request: Request<RuntimeListServicesRequest>,
    ) -> Result<Response<RuntimeListServicesResponse>, Status> {
        // Query dinit via dinitctl list
        let output = tokio::process::Command::new("dinitctl")
            .arg("list")
            .output()
            .await
            .map_err(|e| Status::internal(format!("dinitctl failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut services = Vec::new();

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // dinitctl list format: [{+}] service-name (pid: NNN)
            let state = if line.starts_with("[{+}]") {
                "STARTED"
            } else if line.starts_with("[{-}]") {
                "STOPPED"
            } else {
                "UNKNOWN"
            };
            let name = line
                .split(']')
                .nth(1)
                .unwrap_or("")
                .trim()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            let pid = line
                .find("(pid:")
                .and_then(|i| {
                    line[i + 5..]
                        .split(')')
                        .next()
                        .and_then(|s| s.trim().parse::<u32>().ok())
                })
                .unwrap_or(0);

            if !name.is_empty() {
                services.push(ProtoRuntimeServiceInfo {
                    name,
                    state: state.to_string(),
                    pid,
                    enabled: state == "STARTED",
                    description: String::new(),
                    dependencies: vec![],
                    started_at: None,
                });
            }
        }

        Ok(Response::new(RuntimeListServicesResponse { services }))
    }

    async fn get_service(
        &self,
        request: Request<RuntimeGetServiceRequest>,
    ) -> Result<Response<ProtoRuntimeServiceInfo>, Status> {
        let name = &request.get_ref().service_name;
        let output = tokio::process::Command::new("dinitctl")
            .args(["status", name])
            .output()
            .await
            .map_err(|e| Status::internal(format!("dinitctl status failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let state = if stdout.contains("STARTED") {
            "STARTED"
        } else if stdout.contains("STOPPED") {
            "STOPPED"
        } else {
            "UNKNOWN"
        };

        Ok(Response::new(ProtoRuntimeServiceInfo {
            name: name.clone(),
            state: state.to_string(),
            pid: 0,
            enabled: state == "STARTED",
            description: stdout.trim().to_string(),
            dependencies: vec![],
            started_at: None,
        }))
    }

    async fn stream_metrics(
        &self,
        request: Request<RuntimeStreamMetricsRequest>,
    ) -> Result<Response<Self::StreamMetricsStream>, Status> {
        let interval = std::cmp::max(request.get_ref().interval_seconds, 1) as u64;

        let stream = stream! {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval));
            loop {
                ticker.tick().await;

                // Read /proc/meminfo
                if let Ok(meminfo) = tokio::fs::read_to_string("/proc/meminfo").await {
                    let total = OperationGrpcServer::parse_meminfo_kb(&meminfo, "MemTotal") * 1024;
                    let available = OperationGrpcServer::parse_meminfo_kb(&meminfo, "MemAvailable") * 1024;
                    yield Ok(RuntimeMetricUpdate {
                        category: "memory".to_string(),
                        name: "used_bytes".to_string(),
                        value: (total - available) as f64,
                        unit: "bytes".to_string(),
                        labels: Default::default(),
                        timestamp: Some(ProstTimestamp {
                            seconds: chrono::Utc::now().timestamp(),
                            nanos: 0,
                        }),
                    });
                }
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_interfaces(
        &self,
        _request: Request<RuntimeListInterfacesRequest>,
    ) -> Result<Response<RuntimeListInterfacesResponse>, Status> {
        // Read from /sys/class/net
        let mut interfaces = Vec::new();
        let mut entries = tokio::fs::read_dir("/sys/class/net")
            .await
            .map_err(|e| Status::internal(format!("Cannot read /sys/class/net: {}", e)))?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let base = format!("/sys/class/net/{}", name);

            let mac = tokio::fs::read_to_string(format!("{}/address", base))
                .await
                .unwrap_or_default()
                .trim()
                .to_string();
            let mtu: u32 = tokio::fs::read_to_string(format!("{}/mtu", base))
                .await
                .unwrap_or_default()
                .trim()
                .parse()
                .unwrap_or(0);
            let ifindex: u32 = tokio::fs::read_to_string(format!("{}/ifindex", base))
                .await
                .unwrap_or_default()
                .trim()
                .parse()
                .unwrap_or(0);
            let operstate = tokio::fs::read_to_string(format!("{}/operstate", base))
                .await
                .unwrap_or_default()
                .trim()
                .to_uppercase();

            interfaces.push(ProtoRuntimeNetworkInterface {
                name,
                index: ifindex,
                mac_address: mac,
                state: operstate,
                mtu,
                ipv4_addresses: vec![],
                ipv6_addresses: vec![],
                rx_bytes: 0,
                tx_bytes: 0,
                rx_packets: 0,
                tx_packets: 0,
                driver: String::new(),
                speed_mbps: 0,
            });
        }

        Ok(Response::new(RuntimeListInterfacesResponse { interfaces }))
    }

    async fn get_numa_topology(
        &self,
        _request: Request<RuntimeGetNumaTopologyRequest>,
    ) -> Result<Response<RuntimeGetNumaTopologyResponse>, Status> {
        let mut nodes = Vec::new();

        // Read /sys/devices/system/node/node*/
        if let Ok(mut entries) = tokio::fs::read_dir("/sys/devices/system/node").await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("node") {
                    continue;
                }
                let node_id: u32 = name
                    .strip_prefix("node")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                let meminfo_path = format!("/sys/devices/system/node/{}/meminfo", name);
                let meminfo = tokio::fs::read_to_string(&meminfo_path)
                    .await
                    .unwrap_or_default();
                let mem_total = Self::parse_node_meminfo_kb(&meminfo, "MemTotal") * 1024;
                let mem_free = Self::parse_node_meminfo_kb(&meminfo, "MemFree") * 1024;

                nodes.push(ProtoNumaNode {
                    node_id,
                    cpus: vec![],
                    memory_total_bytes: mem_total,
                    memory_free_bytes: mem_free,
                    memory_used_bytes: mem_total.saturating_sub(mem_free),
                });
            }
        }

        Ok(Response::new(RuntimeGetNumaTopologyResponse { nodes }))
    }
}

impl OperationGrpcServer {
    fn parse_meminfo_kb(meminfo: &str, key: &str) -> u64 {
        meminfo
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(0)
    }

    fn parse_node_meminfo_kb(meminfo: &str, key: &str) -> u64 {
        meminfo
            .lines()
            .find(|l| l.contains(key))
            .and_then(|l| {
                l.split_whitespace()
                    .rev()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(0)
    }

    fn now_ts() -> prost_types::Timestamp {
        let now = Utc::now();
        prost_types::Timestamp {
            seconds: now.timestamp(),
            nanos: now.timestamp_subsec_nanos() as i32,
        }
    }
}

// =============================================================================
// ComponentRegistry Service
// =============================================================================

use crate::proto::registry::{
    component_registry_server::ComponentRegistry, ComponentInfo, ComponentStatus,
    DeregisterRequest, DeregisterResponse, DiscoverRequest, DiscoverResponse, GetComponentRequest,
    GetComponentResponse, HeartbeatRequest, HeartbeatResponse, RegisterRequest, RegisterResponse,
    RegistryEvent, RegistryEventType, WatchRequest,
};

#[tonic::async_trait]
impl ComponentRegistry for OperationGrpcServer {
    type WatchStream = Pin<Box<dyn Stream<Item = Result<RegistryEvent, Status>> + Send + 'static>>;

    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let req = request.into_inner();
        if req.component_id.is_empty() {
            return Err(Status::invalid_argument("component_id must not be empty"));
        }

        let lease_token = Uuid::new_v4().to_string();
        let now = OperationGrpcServer::now_ts();

        let mut inner = self.registry.write().await;

        let is_update = inner.components.contains_key(&req.component_id);
        let info = ComponentInfo {
            component_id: req.component_id.clone(),
            component_type: req.component_type.clone(),
            name: req.name.clone(),
            description: req.description.clone(),
            schema_json: req.schema_json.clone(),
            metadata: req.metadata.clone(),
            capabilities: req.capabilities.clone(),
            endpoint: req.endpoint.clone(),
            version: req.version.clone(),
            status: ComponentStatus::Active as i32,
            registered_at: Some(now.clone()),
            last_heartbeat: Some(now.clone()),
        };

        inner
            .components
            .insert(req.component_id.clone(), info.clone());
        inner
            .leases
            .insert(req.component_id.clone(), lease_token.clone());

        let event_type = if is_update {
            RegistryEventType::RegistryEventUpdated
        } else {
            RegistryEventType::RegistryEventRegistered
        };
        let event = RegistryEvent {
            event_type: event_type as i32,
            component: Some(info),
            timestamp: Some(now.clone()),
        };
        // Ignore send error — no active watchers is fine.
        let _ = inner.watch_tx.send(event);

        info!(
            component_id = %req.component_id,
            component_type = %req.component_type,
            update = is_update,
            "component registered"
        );

        Ok(Response::new(RegisterResponse {
            success: true,
            message: if is_update {
                "updated".to_string()
            } else {
                "registered".to_string()
            },
            lease_token,
            registered_at: Some(now),
        }))
    }

    async fn deregister(
        &self,
        request: Request<DeregisterRequest>,
    ) -> Result<Response<DeregisterResponse>, Status> {
        let req = request.into_inner();
        let mut inner = self.registry.write().await;

        match inner.leases.get(&req.component_id) {
            None => {
                return Ok(Response::new(DeregisterResponse {
                    success: false,
                    message: "component not found".to_string(),
                }))
            }
            Some(stored) if stored != &req.lease_token => {
                return Err(Status::permission_denied("invalid lease token"))
            }
            _ => {}
        }

        let info = inner.components.remove(&req.component_id);
        inner.leases.remove(&req.component_id);

        if let Some(mut component) = info {
            component.status = ComponentStatus::Deregistered as i32;
            let event = RegistryEvent {
                event_type: RegistryEventType::RegistryEventDeregistered as i32,
                component: Some(component),
                timestamp: Some(OperationGrpcServer::now_ts()),
            };
            let _ = inner.watch_tx.send(event);
        }

        info!(component_id = %req.component_id, "component deregistered");

        Ok(Response::new(DeregisterResponse {
            success: true,
            message: "deregistered".to_string(),
        }))
    }

    async fn discover(
        &self,
        request: Request<DiscoverRequest>,
    ) -> Result<Response<DiscoverResponse>, Status> {
        let req = request.into_inner();
        let inner = self.registry.read().await;

        let components: Vec<ComponentInfo> = inner
            .components
            .values()
            .filter(|c| {
                // Type filter
                if !req.component_type.is_empty() && c.component_type != req.component_type {
                    return false;
                }
                // Capability filter
                if !req.capability.is_empty() && !c.capabilities.contains(&req.capability) {
                    return false;
                }
                // Metadata filter
                if !req.metadata_key.is_empty() {
                    match c.metadata.get(&req.metadata_key) {
                        Some(v) if req.metadata_value.is_empty() || v == &req.metadata_value => {}
                        _ => return false,
                    }
                }
                // Stale filter
                if !req.include_stale && c.status == ComponentStatus::Stale as i32 {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        let total = components.len() as u32;
        Ok(Response::new(DiscoverResponse {
            components,
            total_count: total,
        }))
    }

    async fn get_component(
        &self,
        request: Request<GetComponentRequest>,
    ) -> Result<Response<GetComponentResponse>, Status> {
        let req = request.into_inner();
        let inner = self.registry.read().await;
        let component = inner.components.get(&req.component_id).cloned();
        let found = component.is_some();
        Ok(Response::new(GetComponentResponse { component, found }))
    }

    async fn watch(
        &self,
        request: Request<WatchRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let req = request.into_inner();
        let type_filter: Vec<String> = req.component_types.clone();

        let inner = self.registry.read().await;
        let mut rx = inner.watch_tx.subscribe();

        // Collect existing components to replay if requested.
        let existing: Vec<ComponentInfo> = if req.include_existing {
            inner.components.values().cloned().collect()
        } else {
            Vec::new()
        };
        drop(inner);

        let output = stream! {
            // Replay existing registrations first.
            for info in existing {
                if type_filter.is_empty() || type_filter.contains(&info.component_type) {
                    yield Ok(RegistryEvent {
                        event_type: RegistryEventType::RegistryEventRegistered as i32,
                        component: Some(info),
                        timestamp: Some(OperationGrpcServer::now_ts()),
                    });
                }
            }
            // Stream live events.
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let passes_filter = type_filter.is_empty()
                            || event
                                .component
                                .as_ref()
                                .map(|c| type_filter.contains(&c.component_type))
                                .unwrap_or(false);
                        if passes_filter {
                            yield Ok(event);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "Watch stream lagged — skipping events");
                        // Continue rather than closing the stream.
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(output)))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();
        let mut inner = self.registry.write().await;

        match inner.leases.get(&req.component_id) {
            None => {
                // Component not known — tell it to re-register.
                return Ok(Response::new(HeartbeatResponse {
                    acknowledged: false,
                    reregister_required: true,
                    server_time: Some(OperationGrpcServer::now_ts()),
                }));
            }
            Some(stored) if stored != &req.lease_token => {
                return Err(Status::permission_denied("invalid lease token"))
            }
            _ => {}
        }

        let now = OperationGrpcServer::now_ts();
        let was_stale;
        if let Some(info) = inner.components.get_mut(&req.component_id) {
            was_stale = info.status == ComponentStatus::Stale as i32;
            info.last_heartbeat = Some(now.clone());
            if was_stale {
                info.status = ComponentStatus::Active as i32;
            }
        } else {
            return Ok(Response::new(HeartbeatResponse {
                acknowledged: false,
                reregister_required: true,
                server_time: Some(now),
            }));
        }

        if was_stale {
            if let Some(info) = inner.components.get(&req.component_id).cloned() {
                let event = RegistryEvent {
                    event_type: RegistryEventType::RegistryEventRecovered as i32,
                    component: Some(info),
                    timestamp: Some(now.clone()),
                };
                let _ = inner.watch_tx.send(event);
                info!(component_id = %req.component_id, "component recovered from stale");
            }
        }

        debug!(component_id = %req.component_id, "heartbeat acknowledged");

        Ok(Response::new(HeartbeatResponse {
            acknowledged: true,
            reregister_required: false,
            server_time: Some(now),
        }))
    }
}
