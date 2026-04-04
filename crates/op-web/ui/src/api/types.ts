// API Types for operation-dbus backend

export interface HealthResponse {
  status: "healthy" | "degraded" | "unhealthy";
  uptime_seconds: number;
  version: string;
  components: Record<string, { status: string; latency_ms?: number }>;
}

export interface StatusResponse {
  node_count: number;
  service_count: number;
  tool_count: number;
  agent_count: number;
  active_sessions: number;
  bus_connections: { system: boolean; session: boolean };
  llm_status: LlmStatus;
}

export interface ToolInfo {
  name: string;
  description: string;
  namespace: string;
  category: string;
  tags: string[];
  input_schema: Record<string, unknown>;
  output_schema?: Record<string, unknown>;
}

export interface ToolExecutionRequest {
  arguments: Record<string, unknown>;
}

export interface ToolExecutionResult {
  tool_name: string;
  success: boolean;
  result: unknown;
  duration_ms: number;
  error?: string;
  timestamp: string;
}

export interface LlmStatus {
  provider: string;
  model: string;
  available: boolean;
  route: string;
}

export interface LlmModel {
  id: string;
  name: string;
  provider: string;
  context_length: number;
  capabilities: string[];
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "tool" | "system";
  content: string;
  tool_calls?: ToolCall[];
  tool_result?: unknown;
  timestamp: string;
  session_id: string;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

export interface ChatRequest {
  message: string;
  session_id?: string;
  model?: string;
}

export interface AgentInfo {
  id: string;
  name: string;
  description: string;
  status: "active" | "idle" | "error";
  model: string;
  capabilities: string[];
  tools: string[];
  sessions: number;
}

export interface ConfigSection {
  key: string;
  label: string;
  description: string;
  values: Record<string, unknown>;
  schema?: Record<string, unknown>;
}

export interface SSEEvent {
  id: string;
  type: "state_update" | "audit_event" | "system_stats" | "message" | "error" | string;
  data: unknown;
  timestamp: string;
}

export interface StateEntry {
  key: string;
  plugin: string;
  object_type: string;
  value: unknown;
  updated_at: string;
}

export interface LogEntry {
  id: string;
  level: "error" | "warn" | "info" | "debug" | "trace";
  source: string;
  message: string;
  metadata?: Record<string, unknown>;
  timestamp: string;
  node_id?: string;
}

export interface SecurityInfo {
  tls_enabled: boolean;
  auth_mode: string;
  active_sessions: number;
  policy_count: number;
  recent_events: AuditEvent[];
}

export interface AuditEvent {
  id: string;
  action: string;
  actor: string;
  target: string;
  result: "success" | "failure";
  timestamp: string;
  details?: Record<string, unknown>;
}
