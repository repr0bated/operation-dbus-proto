/**
 * Hand-written TypeScript types mirroring registry.proto (operation.registry.v1).
 */

import type { ProtobufStruct } from "../google/protobuf/struct";

export enum ComponentStatus {
  UNSPECIFIED = 0,
  ACTIVE = 1,
  STALE = 2,
  DEREGISTERED = 3,
}

export enum RegistryEventType {
  UNSPECIFIED = 0,
  REGISTERED = 1,
  DEREGISTERED = 2,
  UPDATED = 3,
  STALE = 4,
  RECOVERED = 5,
}

export interface ComponentInfo {
  componentId: string;
  componentType: string;
  name: string;
  description: string;
  schemaJson: string;
  metadata: Record<string, string>;
  capabilities: string[];
  endpoint: string;
  version: string;
  status: ComponentStatus;
  registeredAt: string;
  lastHeartbeat: string;
}

export interface RegisterRequest {
  componentId: string;
  componentType: string;
  name: string;
  description: string;
  schemaJson: string;
  metadata: Record<string, string>;
  capabilities: string[];
  endpoint: string;
  version: string;
  heartbeatIntervalSeconds: number;
}

export interface RegisterResponse {
  success: boolean;
  message: string;
  leaseToken: string;
  registeredAt: string;
}

export interface DiscoverRequest {
  componentType: string;
  capability: string;
  metadataKey: string;
  metadataValue: string;
  includeStale: boolean;
}

export interface DiscoverResponse {
  components: ComponentInfo[];
  totalCount: number;
}

export interface WatchRequest {
  componentTypes: string[];
  includeExisting: boolean;
}

export interface RegistryEvent {
  eventType: RegistryEventType;
  component: ComponentInfo;
  timestamp: string;
}

export interface HeartbeatRequest {
  componentId: string;
  leaseToken: string;
  statusPayload?: ProtobufStruct;
}

export interface HeartbeatResponse {
  acknowledged: boolean;
  reregisterRequired: boolean;
  serverTime: string;
}
