// op-dbus REST API types — mirrors op-core Rust types

// Same-origin when served by op-web; override with VITE_API_BASE for remote dev.
export const API_BASE = import.meta.env.VITE_API_BASE ?? "/api";

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

// ── Plugin State Types ───────────────────────────────────────

/**
 * Child object summary — used for repeat binding in json-render specs.
 * Maps to Rust `ChildSummary` in json_render.rs.
 */
export interface ChildSummary {
  id: string;
  label: string;
  status: ChildStatus;
  created_at: number;
  updated_at: number;
  /** Freeform payload keyed by plugin-specific field names */
  data: Record<string, unknown>;
  /** Per-item actions the UI can invoke */
  actions: ChildAction[];
}

export type ChildStatus = 'pending' | 'active' | 'completed' | 'failed' | 'cancelled';

export interface ChildAction {
  name: string;
  label: string;
  variant?: 'default' | 'destructive' | 'outline' | 'ghost';
  confirm?: string;
}

/**
 * Bounded collection of child objects for a plugin.
 * Maps to Rust `BoundedChildren` in json_render.rs.
 */
export interface BoundedChildren {
  children: ChildSummary[];
  total: number;
  window_size: number;
  cursor?: string;
}

/**
 * Plugin projection state — the full present-state JSON object
 * stored in /dev/shm/opdbus/state/<plugin>.json
 */
export type PluginState = Record<string, unknown>;

/**
 * Response from GET /api/ui-model/state
 */
export interface PluginStateResponse {
  plugins: string[];
  state: Record<string, PluginState>;
}
