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

export interface SubscribeRequest {
  eventTypes: string[];
  sessionId?: string;
}

export interface McpEvent {
  eventType: string;
  dataJson: string;
  timestamp: number;
  sequence: number;
}

export interface HealthRequest {}

export interface HealthResponse {
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

export enum FileOperation {
  READ = 0,
  WRITE = 1,
  DELETE = 2,
  LIST = 3,
}

export interface FileMode {
  mode: number;
}

export interface FileSystemArgs {
  path: string;
  content?: string;
  operation: FileOperation;
  mode?: FileMode;
}

export interface NetworkArgs {
  url: string;
  method: string;
  headers: Record<string, string>;
  body?: string;
}

export interface DatabaseArgs {
  query: string;
  parameters: Record<string, string>;
}

export interface ShellArgs {
  command: string;
  args: string[];
  env: Record<string, string>;
  workingDir?: string;
}

export interface ToolArguments {
  filesystem?: FileSystemArgs;
  network?: NetworkArgs;
  database?: DatabaseArgs;
  shell?: ShellArgs;
  generic?: ProtobufStruct;
}

export interface CallToolRequest {
  toolName: string;
  arguments: ToolArguments;
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
