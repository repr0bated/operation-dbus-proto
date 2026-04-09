/**
 * TypeScript types for operation.v1 StateSync service.
 * Generated from: crates/op-grpc-bridge/proto/operation.proto
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

// ── Messages ────────────────────────────────────────────────────────────────

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
