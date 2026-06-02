/**
 * TypeScript types for operation.v1 PluginService.
 * Generated from: crates/op-grpc-bridge/proto/operation.proto
 */

import type { ProtobufValue } from "../google/protobuf/struct";
import type { MutationError } from "./state-sync";

// ── Messages ────────────────────────────────────────────────────────────────

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

export interface GetPropertyRequest {
  pluginId: string;
  objectPath: string;
  interfaceName: string;
  propertyName: string;
}

export interface GetPropertyResponse {
  value?: ProtobufValue;
  readOnly: boolean;
}

export interface SetPropertyRequest {
  pluginId: string;
  objectPath: string;
  interfaceName: string;
  propertyName: string;
  value?: ProtobufValue;
  actorId: string;
  capabilityId: string;
}

export interface SetPropertyResponse {
  success: boolean;
  eventId: number;
  eventHash: string;
  error?: MutationError;
}

export interface SubscribeSignalsRequest {
  pluginId: string;
  signalNames: string[];
  objectPath: string;
}

export interface Signal {
  pluginId: string;
  objectPath: string;
  interfaceName: string;
  signalName: string;
  arguments: ProtobufValue[];
  timestamp: string;
}
