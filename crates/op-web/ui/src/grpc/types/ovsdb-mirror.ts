/**
 * TypeScript types for operation.v1 OvsdbMirror service.
 * Generated from: crates/op-grpc-bridge/proto/operation.proto
 */

import type { ProtobufStruct } from "../google/protobuf/struct";

// ── Messages ────────────────────────────────────────────────────────────────

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

export interface OvsdbEchoRequest {
  payload: string[];
}

export interface OvsdbEchoResponse {
  payload: string[];
}

export interface OvsdbDumpDbRequest {
  database: string;
}

export interface OvsdbDumpDbResponse {
  dumpJson: string;
}

export interface OvsdbGetBridgeStateResponse {
  bridges: OvsdbBridge[];
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
