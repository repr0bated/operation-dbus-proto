/**
 * TypeScript types for operation.v1 EventChainService.
 * Generated from: crates/op-grpc-bridge/proto/operation.proto
 */

import type { ProtobufStruct } from "../google/protobuf/struct";
import type { Decision, DenyReason } from "./state-sync";

// ── Messages ────────────────────────────────────────────────────────────────

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

export interface GetProofRequest {
  eventId: number;
}

export interface GetProofResponse {
  eventHash: string;
  siblings: MerkleProofSibling[];
  root: string;
  batchFirstEventId: number;
  batchLastEventId: number;
}

export interface MerkleProofSibling {
  hash: string;
  isRight: boolean;
}

export interface ProveTagImmutabilityRequest {
  tag: string;
  pluginId: string;
}

export interface ProveTagImmutabilityResponse {
  tag: string;
  isImmutable: boolean;
  violationEventIds: number[];
  totalEventsChecked: number;
}

export interface GetSnapshotRequest {
  snapshotId: string;
}

export interface GetSnapshotResponse {
  snapshot: Snapshot;
}

export interface CreateSnapshotRequest {
  pluginId: string;
}

export interface CreateSnapshotResponse {
  snapshot: Snapshot;
}

export interface Snapshot {
  snapshotId: string;
  atEventId: number;
  pluginId: string;
  schemaVersion: string;
  stubHash: string;
  immutableWrappersHash: string;
  tunablePatchHash: string;
  effectiveHash: string;
  timestamp: string;
  state: ProtobufStruct;
}
