/**
 * Hand-written TypeScript types mirroring operation.proto (operation.v1).
 * These map 1:1 to the Protobuf messages and are used by the gRPC-Web clients.
 */

import type { ProtobufStruct, ProtobufValue } from "../google/protobuf/struct";

// ── Enums ───────────────────────────────────────────────────────────────────

export enum ChangeType {
  UNSPECIFIED = 0,
  PROPERTY_SET = 1,
  PROPERTY_DELETE = 2,
  METHOD_CALL = 3,
  SIGNAL = 4,
  OBJECT_ADDED = 5,
  OBJECT_REMOVED = 6,
  SCHEMA_MIGRATION = 7,
}

export enum OperationType {
  UNSPECIFIED = 0,
  SET_PROPERTY = 1,
  CALL_METHOD = 2,
  APPLY_PATCH = 3,
}

export enum ErrorCode {
  UNSPECIFIED = 0,
  NOT_FOUND = 1,
  PERMISSION_DENIED = 2,
  VALIDATION_FAILED = 3,
  READ_ONLY = 4,
  TAG_LOCKED = 5,
  INTERNAL = 6,
}

export enum Decision {
  UNSPECIFIED = 0,
  ALLOW = 1,
  DENY = 2,
}

// ── StateSync ───────────────────────────────────────────────────────────────

export interface SubscribeRequest {
  pluginIds: string[];
  pathPatterns: string[];
  tags: string[];
  includeInitialState: boolean;
}

export interface StateChange {
  changeId: string;
  eventId: number;
  pluginId: string;
  objectPath: string;
  changeType: ChangeType;
  memberName: string;
  oldValue?: ProtobufValue;
  newValue?: ProtobufValue;
  tagsTouched: string[];
  eventHash: string;
  timestamp: string;
  actorId: string;
}

export interface MutateRequest {
  pluginId: string;
  objectPath: string;
  operation: OperationType;
  memberName: string;
  value?: ProtobufValue;
  actorId: string;
  capabilityId: string;
  idempotencyKey: string;
}

export interface MutateResponse {
  success: boolean;
  eventId: number;
  eventHash: string;
  result?: ProtobufValue;
  error?: MutationError;
  effectiveHash: string;
}

export interface MutationError {
  code: ErrorCode;
  message: string;
  denyReason?: DenyReason;
}

export interface DenyReason {
  tagLock?: { tag: string; wrapperId: string };
  constraintFail?: { constraint: string; message: string };
  capabilityMissing?: { capability: string };
  readOnlyViolation?: { field: string };
}

export interface GetStateRequest {
  pluginId: string;
  objectPath: string;
}

export interface GetStateResponse {
  state: ProtobufStruct;
  effectiveHash: string;
  atEventId: number;
}

export interface BatchMutateRequest {
  mutations: MutateRequest[];
  atomic: boolean;
  actorId: string;
}

export interface BatchMutateResponse {
  success: boolean;
  results: MutateResponse[];
  failedIndex: number;
}

// ── PluginService ───────────────────────────────────────────────────────────

export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  dbusPath: string;
  interfaces: string[];
  tags: string[];
}

export interface ListPluginsResponse {
  plugins: PluginInfo[];
}

export interface GetSchemaRequest {
  pluginId: string;
  format: string;
}

export interface GetSchemaResponse {
  schemaJson: string;
  dialect: string;
  version: string;
}

export interface CallMethodRequest {
  pluginId: string;
  objectPath: string;
  interfaceName: string;
  methodName: string;
  arguments: ProtobufValue[];
  actorId: string;
  capabilityId: string;
}

export interface CallMethodResponse {
  success: boolean;
  result?: ProtobufValue;
  eventId: number;
  eventHash: string;
  error?: MutationError;
}

export interface Signal {
  pluginId: string;
  objectPath: string;
  interfaceName: string;
  signalName: string;
  arguments: ProtobufValue[];
  timestamp: string;
}

// ── EventChainService ───────────────────────────────────────────────────────

export interface ChainEvent {
  eventId: number;
  prevHash: string;
  eventHash: string;
  timestamp: string;
  actorId: string;
  capabilityId: string;
  pluginId: string;
  schemaVersion: string;
  operationType: string;
  target: string;
  tagsTouched: string[];
  decision: Decision;
  denyReason?: DenyReason;
  inputPatchHash: string;
  resultEffectiveHash: string;
}

export interface GetEventsRequest {
  fromEventId: number;
  toEventId: number;
  limit: number;
  pluginId: string;
  tags: string[];
  decisionFilter: Decision;
}

export interface GetEventsResponse {
  events: ChainEvent[];
  hasMore: boolean;
}

export interface SubscribeEventsRequest {
  fromEventId: number;
  pluginId: string;
  tags: string[];
}

export interface VerifyChainRequest {
  fromEventId: number;
  toEventId: number;
}

export interface VerifyChainResponse {
  valid: boolean;
  eventsVerified: number;
  batchesVerified: number;
  errors: string[];
}

// ── OvsdbMirror ─────────────────────────────────────────────────────────────

export interface OvsdbListDbsResponse {
  databases: string[];
}

export interface OvsdbGetSchemaResponse {
  schemaJson: string;
  name: string;
  version: string;
}

export interface OvsdbTransactRequest {
  database: string;
  operationsJson: string;
  actorId: string;
}

export interface OvsdbTransactResponse {
  success: boolean;
  resultsJson: string;
  eventId: number;
  error: string;
}

export interface OvsdbMonitorRequest {
  database: string;
  monitorRequestsJson: string;
}

export interface OvsdbUpdate {
  table: string;
  uuid: string;
  oldRow?: ProtobufStruct;
  newRow?: ProtobufStruct;
  timestamp: string;
}

export interface OvsdbBridge {
  name: string;
  datapathType: string;
  failMode: string;
  stpEnable: boolean;
  mcastSnoopingEnable: boolean;
  otherConfig: Record<string, string>;
  ports: OvsdbPort[];
}

export interface OvsdbPort {
  name: string;
  tag: number;
  trunks: number[];
  vlanMode: string;
  bondMode: string;
  interfaces: OvsdbInterface[];
}

export interface OvsdbInterface {
  name: string;
  type: string;
  macInUse: string;
  mac: string;
  adminState: string;
  linkState: string;
  options: Record<string, string>;
}

export interface OvsdbGetBridgeStateResponse {
  bridges: OvsdbBridge[];
}

// ── RuntimeMirror ───────────────────────────────────────────────────────────

export interface RuntimeSystemInfo {
  hostname: string;
  kernelVersion: string;
  uptimeSeconds: number;
  bootTimestamp: number;
  cpuCount: number;
  memoryTotalBytes: number;
  memoryAvailableBytes: number;
  memoryUsedBytes: number;
  initSystem: string;
  arch: string;
  queriedAt: string;
}

export interface RuntimeServiceInfo {
  name: string;
  state: string;
  pid: number;
  enabled: boolean;
  description: string;
  dependencies: string[];
  startedAt: string;
}

export interface RuntimeListServicesResponse {
  services: RuntimeServiceInfo[];
}

export interface RuntimeStreamMetricsRequest {
  intervalSeconds: number;
  categories: string[];
}

export interface RuntimeMetricUpdate {
  category: string;
  name: string;
  value: number;
  unit: string;
  labels: Record<string, string>;
  timestamp: string;
}

export interface RuntimeNetworkInterface {
  name: string;
  index: number;
  macAddress: string;
  state: string;
  mtu: number;
  ipv4Addresses: string[];
  ipv6Addresses: string[];
  rxBytes: number;
  txBytes: number;
  rxPackets: number;
  txPackets: number;
  driver: string;
  speedMbps: number;
}

export interface RuntimeListInterfacesResponse {
  interfaces: RuntimeNetworkInterface[];
}

export interface NumaNode {
  nodeId: number;
  cpus: number[];
  memoryTotalBytes: number;
  memoryFreeBytes: number;
  memoryUsedBytes: number;
}

export interface RuntimeNumaTopologyResponse {
  nodes: NumaNode[];
}
