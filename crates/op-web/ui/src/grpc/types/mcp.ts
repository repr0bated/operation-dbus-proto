/**
 * TypeScript types for op.mcp.v1 McpService.
 * Generated from: crates/op-mcp/proto/mcp.proto
 */

import type { ProtobufStruct, ProtobufValue } from "../google/protobuf/struct";

// ── Enums ───────────────────────────────────────────────────────────────────

export enum ServerMode {
  UNKNOWN = 0,
  COMPACT = 1,
  AGENTS = 2,
  FULL = 3,
}

export enum ParameterType {
  STRING = 0,
  INTEGER = 1,
  NUMBER = 2,
  BOOLEAN = 3,
  ARRAY = 4,
  OBJECT = 5,
}

// ── Messages ────────────────────────────────────────────────────────────────

export interface McpRequest {
  jsonrpc: string;
  id?: string;
  method: string;
  params?: ProtobufStruct;
}

export interface McpResponse {
  jsonrpc: string;
  id?: string;
  result?: ProtobufStruct;
  error?: McpError;
}

export interface McpError {
  code: number;
  message: string;
  data?: ProtobufStruct;
}

export interface McpSubscribeRequest {
  eventTypes: string[];
  sessionId?: string;
}

export interface McpEvent {
  eventType: string;
  dataJson: string;
  timestamp: number;
  sequence: number;
}

export interface McpHealthResponse {
  healthy: boolean;
  version: string;
  serverName: string;
  mode: ServerMode;
  connectedAgents: string[];
  uptimeSecs: number;
}

export interface InitializeRequest {
  clientName: string;
  clientVersion?: string;
  sessionId?: string;
  capabilities: string[];
}

export interface InitializeResponse {
  protocolVersion: string;
  serverName: string;
  serverVersion: string;
  capabilities: string[];
  startedAgents: string[];
  sessionId: string;
}

export interface ListToolsRequest {
  category?: string;
  query?: string;
  limit: number;
  offset: number;
}

export interface ListToolsResponse {
  tools: ToolInfo[];
  total: number;
  hasMore: boolean;
}

export interface ToolInfo {
  name: string;
  description: string;
  inputSchema: ToolSchema;
  category?: string;
  tags: string[];
}

export interface ToolSchema {
  parameters: ToolParameter[];
  required: string[];
}

export interface ToolParameter {
  name: string;
  type: ParameterType;
  description: string;
  defaultValue?: ProtobufValue;
  enumValues: string[];
}

export interface CallToolRequest {
  toolName: string;
  arguments: ProtobufStruct;
  sessionId?: string;
  timeoutMs?: number;
}

export interface CallToolResponse {
  success: boolean;
  result: ProtobufStruct;
  error?: string;
  durationMs: number;
}

export interface ToolOutput {
  outputType: number;
  content: string;
  sequence: number;
  isFinal: boolean;
  exitCode?: number;
}
