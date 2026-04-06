/**
 * Operation-DBUS API types — mirrors the Rust backend data model.
 */

// ── D-Bus Core ──────────────────────────────────────────
export interface DbusService {
  name: string;
  uniqueName: string | null;
  pid: number | null;
  cmdline: string | null;
  bus: "system" | "session";
  isActivatable: boolean;
  interfaces: string[];
  objectPaths: string[];
}

export interface DbusMethod {
  name: string;
  inSignature: string;
  outSignature: string;
}

export interface DbusSignal {
  name: string;
  signature: string;
}

export interface DbusProperty {
  name: string;
  signature: string;
  access: "read" | "write" | "readwrite";
  value?: unknown;
}

// ── Tools ───────────────────────────────────────────────
export interface Tool {
  id: string;
  name: string;
  description: string;
  inputSchema: JsonSchema;
  outputSchema?: JsonSchema;
  category: string;
  enabled: boolean;
  source: "builtin" | "dbus" | "mcp" | "custom";
}

export interface ToolExecution {
  id: string;
  toolId: string;
  input: Record<string, unknown>;
  output: unknown;
  status: "pending" | "running" | "completed" | "error";
  error?: string;
  duration?: number;
  timestamp: string;
}

// ── Agents ──────────────────────────────────────────────
export interface Agent {
  id: string;
  name: string;
  status: "running" | "idle" | "error" | "stopped";
  model: string;
  sessionKey: string;
  tools: string[];
  lastActive: string | null;
  errorMessage?: string;
  identity?: { name?: string; emoji?: string; avatar?: string };
}

// ── LLM ─────────────────────────────────────────────────
export interface LlmProvider {
  id: string;
  name: string;
  model: string;
  endpoint: string;
  status: "connected" | "disconnected" | "error";
  tokenUsage: { prompt: number; completion: number; total: number };
}

export interface LlmModel {
  id: string;
  name: string;
  provider: string;
  contextWindow: number;
  active: boolean;
}

// ── Sessions & Chat ─────────────────────────────────────
export interface Session {
  key: string;
  agentId: string;
  channel: string;
  createdAt: string;
  lastMessageAt: string | null;
  messageCount: number;
  tokenUsage: { prompt: number; completion: number };
  label?: string;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  timestamp: string;
  toolCalls?: ToolCall[];
  thinking?: string;
  metadata?: Record<string, unknown>;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  result?: unknown;
  status: "pending" | "running" | "completed" | "error";
  duration?: number;
}

// ── System ──────────────────────────────────────────────
export interface HealthSnapshot {
  status: "healthy" | "degraded" | "error";
  uptimeMs: number;
  version: string;
  services: number;
  agents: number;
  activeSessions: number;
  memoryMb: number;
  cpuPercent: number;
}

export interface StatusSummary {
  connected: boolean;
  gateway: string;
  authMode: string;
  securityAudit?: { summary?: Record<string, number> };
}

export interface LogEntry {
  id: string;
  time: string;
  level: LogLevel;
  subsystem: string;
  message: string;
  raw: string;
  metadata?: Record<string, unknown>;
}

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error" | "fatal";

export interface EventLogEntry {
  id: string;
  event: string;
  ts: number;
  payload: unknown;
}

// ── Config ──────────────────────────────────────────────
export interface ConfigSnapshot {
  config: Record<string, unknown>;
  schema?: JsonSchema;
  version: string;
  lastModified: string;
}

// ── JSON Schema ─────────────────────────────────────────
export interface JsonSchema {
  type?: string;
  title?: string;
  description?: string;
  properties?: Record<string, JsonSchema>;
  items?: JsonSchema;
  required?: string[];
  enum?: unknown[];
  default?: unknown;
  oneOf?: JsonSchema[];
  anyOf?: JsonSchema[];
  additionalProperties?: boolean | JsonSchema;
  format?: string;
  [key: string]: unknown;
}

// ── SSE Events ──────────────────────────────────────────
export type SseEventType =
  | "health"
  | "log"
  | "state_update"
  | "audit_event"
  | "system_stats"
  | "message"
  | "service_change"
  | "agent_status"
  | "tool_call"
  | "registry_event";

export interface SseEvent<T = unknown> {
  type: SseEventType;
  timestamp: string;
  data: T;
}
