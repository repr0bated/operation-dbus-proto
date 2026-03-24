// op-dbus REST API types — mirrors op-core Rust types

export const API_BASE = "https://mail.3tched.com/api";

// ── D-Bus Types ──────────────────────────────────────────────

export type BusType = "system" | "session";

export interface ServiceInfo {
  name: string;
  bus_type: BusType;
  activatable: boolean;
  active: boolean;
  pid?: number;
  uid?: number;
}

// ── Tool Types ───────────────────────────────────────────────

export interface ToolDefinition {
  name: string;
  description: string;
  input_schema: Record<string, unknown>;
  schema_version?: string;
  category?: string;
  tags?: string[];
  namespace?: string;
}

export interface ToolRequest {
  id: string;
  tool_name: string;
  arguments: Record<string, unknown>;
  timeout_ms?: number;
}

export interface ToolResult {
  id: string;
  success: boolean;
  content: unknown;
  error?: string;
  execution_time_ms: number;
}

// ── Agent Types ──────────────────────────────────────────────

export type AgentStatus = "idle" | "running" | "paused" | "error" | "stopped";

export interface AgentDefinition {
  id: string;
  name: string;
  description: string;
  capabilities: string[];
  tools: string[];
  model?: string;
  config?: Record<string, unknown>;
  status?: AgentStatus;
}

// ── Chat Types ───────────────────────────────────────────────

export type ChatRole = "user" | "assistant" | "system" | "tool";

export interface ToolCall {
  id: string;
  tool_name: string;
  arguments: Record<string, unknown>;
  result?: ToolResult;
}

export interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  timestamp: string;
  tool_calls?: ToolCall[];
  metadata?: Record<string, unknown>;
}

// ── Health Types ─────────────────────────────────────────────

export type ComponentStatus = "healthy" | "degraded" | "unhealthy" | "unknown";

export interface ComponentHealth {
  name: string;
  status: ComponentStatus;
  message?: string;
  last_check: string;
}

export interface HealthStatus {
  healthy: boolean;
  version: string;
  uptime_secs: number;
  components: Record<string, ComponentHealth>;
}

// ── Status Types ─────────────────────────────────────────────

export interface SystemStatus {
  health: HealthStatus;
  tools_count?: number;
  agents_count?: number;
  services?: ServiceInfo[];
  [key: string]: unknown;
}

// ── LLM Types ────────────────────────────────────────────────

export interface LlmProvider {
  name: string;
  enabled: boolean;
  models: string[];
  status?: string;
}

export interface LlmStatus {
  active_provider?: string;
  active_model?: string;
  providers: LlmProvider[];
  [key: string]: unknown;
}

export interface LlmModel {
  id: string;
  name: string;
  provider: string;
  context_length?: number;
  [key: string]: unknown;
}

// ── Chat Request/Response ────────────────────────────────────

export interface ChatRequest {
  message: string;
  session_id?: string;
}

export interface ChatResponse {
  message: ChatMessage;
  session_id: string;
}
