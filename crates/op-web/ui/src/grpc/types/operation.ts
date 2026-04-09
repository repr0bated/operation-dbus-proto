/**
 * Re-export barrel for backward compatibility.
 * Types are now split into domain-specific files.
 */

// StateSync types (core enums + state mutations)
export {
  ChangeType, OperationType, ErrorCode, Decision,
  type SubscribeRequest, type StateChange, type MutateRequest, type MutateResponse,
  type MutationError, type DenyReason, type GetStateRequest, type GetStateResponse,
  type BatchMutateRequest, type BatchMutateResponse,
} from "./state-sync";

// PluginService types
export {
  type PluginInfo, type ListPluginsResponse, type GetSchemaRequest, type GetSchemaResponse,
  type CallMethodRequest, type CallMethodResponse,
  type GetPropertyRequest, type GetPropertyResponse,
  type SetPropertyRequest, type SetPropertyResponse,
  type SubscribeSignalsRequest, type Signal,
} from "./plugin-service";

// EventChainService types
export {
  type ChainEvent, type GetEventsRequest, type GetEventsResponse,
  type SubscribeEventsRequest, type VerifyChainRequest, type VerifyChainResponse,
  type GetProofRequest, type GetProofResponse, type MerkleProofSibling,
  type ProveTagImmutabilityRequest, type ProveTagImmutabilityResponse,
  type GetSnapshotRequest, type GetSnapshotResponse,
  type CreateSnapshotRequest, type CreateSnapshotResponse, type Snapshot,
} from "./event-chain";

// OvsdbMirror types
export {
  type OvsdbListDbsResponse, type OvsdbGetSchemaResponse,
  type OvsdbTransactRequest, type OvsdbTransactResponse,
  type OvsdbMonitorRequest, type OvsdbUpdate,
  type OvsdbEchoRequest, type OvsdbEchoResponse,
  type OvsdbDumpDbRequest, type OvsdbDumpDbResponse,
  type OvsdbGetBridgeStateResponse,
  type OvsdbBridge, type OvsdbPort, type OvsdbInterface,
} from "./ovsdb-mirror";

// RuntimeMirror types
export {
  type RuntimeSystemInfo, type RuntimeServiceInfo, type RuntimeListServicesResponse,
  type RuntimeStreamMetricsRequest, type RuntimeMetricUpdate,
  type RuntimeNetworkInterface, type RuntimeListInterfacesResponse,
  type NumaNode, type RuntimeNumaTopologyResponse,
} from "./runtime-mirror";
