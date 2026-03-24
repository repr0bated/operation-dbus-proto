import { API_BASE } from "./types";
import type {
  HealthStatus,
  SystemStatus,
  ToolDefinition,
  ToolResult,
  AgentDefinition,
  LlmStatus,
  LlmModel,
  ChatResponse,
} from "./types";

class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const url = `${API_BASE}${path}`;
  const res = await fetch(url, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options?.headers,
    },
  });

  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new ApiError(res.status, text);
  }

  return res.json();
}

// ── Health ────────────────────────────────────────────────────

export async function fetchHealth(): Promise<HealthStatus> {
  return request<HealthStatus>("/health");
}

// ── Status ────────────────────────────────────────────────────

export async function fetchStatus(): Promise<SystemStatus> {
  return request<SystemStatus>("/status");
}

// ── Tools ─────────────────────────────────────────────────────

export async function fetchTools(): Promise<ToolDefinition[]> {
  const res = await request<{ tools: ToolDefinition[] } | ToolDefinition[]>("/tools");
  return Array.isArray(res) ? res : res.tools ?? [];
}

export async function fetchTool(name: string): Promise<ToolDefinition> {
  return request<ToolDefinition>(`/tools/${encodeURIComponent(name)}`);
}

export async function executeTool(
  toolName: string,
  args: Record<string, unknown>
): Promise<ToolResult> {
  return request<ToolResult>("/tool", {
    method: "POST",
    body: JSON.stringify({ tool_name: toolName, arguments: args }),
  });
}

// ── Agents ────────────────────────────────────────────────────

export async function fetchAgents(): Promise<AgentDefinition[]> {
  const res = await request<{ agents: AgentDefinition[] } | AgentDefinition[]>("/agents");
  return Array.isArray(res) ? res : res.agents ?? [];
}

export async function fetchAgent(id: string): Promise<AgentDefinition> {
  return request<AgentDefinition>(`/agents/${encodeURIComponent(id)}`);
}

export async function spawnAgent(agentType: string): Promise<AgentDefinition> {
  return request<AgentDefinition>("/agents", {
    method: "POST",
    body: JSON.stringify({ agent_type: agentType }),
  });
}

// ── LLM ───────────────────────────────────────────────────────

export async function fetchLlmStatus(): Promise<LlmStatus> {
  return request<LlmStatus>("/llm/status");
}

export async function fetchLlmProviders(): Promise<unknown[]> {
  const res = await request<{ providers: unknown[] } | unknown[]>("/llm/providers");
  return Array.isArray(res) ? res : (res as { providers: unknown[] }).providers ?? [];
}

export async function fetchLlmModels(): Promise<LlmModel[]> {
  const res = await request<{ models: LlmModel[] } | LlmModel[]>("/llm/models");
  return Array.isArray(res) ? res : res.models ?? [];
}

export async function switchModel(model: string): Promise<unknown> {
  return request("/llm/model", {
    method: "POST",
    body: JSON.stringify({ model }),
  });
}

// ── Chat Sessions ─────────────────────────────────────────────────

export interface ChatSession {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  message_count: number;
}

export async function listChatSessions(): Promise<ChatSession[]> {
  const res = await request<{ sessions: ChatSession[] } | ChatSession[]>("/chat/sessions");
  return Array.isArray(res) ? res : res.sessions ?? [];
}

export async function createChatSession(title?: string): Promise<ChatSession> {
  return request<ChatSession>("/chat/sessions", {
    method: "POST",
    body: JSON.stringify({ title }),
  });
}

export async function deleteChatSession(sessionId: string): Promise<void> {
  await request(`/chat/sessions/${encodeURIComponent(sessionId)}`, {
    method: "DELETE",
  });
}

export async function getChatSession(sessionId: string): Promise<ChatSession> {
  return request<ChatSession>(`/chat/sessions/${encodeURIComponent(sessionId)}`);
}

// ── Chat ──────────────────────────────────────────────────────

export async function sendChat(
  message: string,
  sessionId?: string
): Promise<ChatResponse> {
  return request<ChatResponse>("/chat", {
    method: "POST",
    body: JSON.stringify({ message, session_id: sessionId }),
  });
}

export function streamChat(
  message: string,
  sessionId?: string,
  onChunk: (text: string) => void = () => { },
  onDone: () => void = () => { }
): AbortController {
  const controller = new AbortController();

  fetch(`${API_BASE}/chat/stream`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ message, session_id: sessionId }),
    signal: controller.signal,
  })
    .then(async (res) => {
      if (!res.ok || !res.body) {
        onDone();
        return;
      }
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        onChunk(decoder.decode(value, { stream: true }));
      }
      onDone();
    })
    .catch(() => onDone());

  return controller;
}

// ── Admin ─────────────────────────────────────────────────────

export async function fetchConfig(): Promise<Record<string, unknown>> {
  return request<Record<string, unknown>>("/admin/config");
}

export async function fetchSystemPrompt(): Promise<string> {
  const res = await request<{ prompt: string } | string>("/admin/prompt");
  return typeof res === "string" ? res : res.prompt ?? "";
}

// ── SSE Events ────────────────────────────────────────────────

export function subscribeEvents(
  onEvent: (event: MessageEvent) => void,
  onError?: (err: Event) => void
): EventSource {
  const source = new EventSource(`${API_BASE}/events`);
  source.onmessage = onEvent;
  if (onError) source.onerror = onError;
  return source;
}

// ── WebSocket ─────────────────────────────────────────────────

export function connectWebSocket(
  onMessage: (data: unknown) => void,
  onError?: (err: Event) => void
): WebSocket {
  const wsUrl = API_BASE.replace(/^http/, "ws").replace(/\/api$/, "/ws");
  const ws = new WebSocket(wsUrl);
  ws.onmessage = (e) => {
    try {
      onMessage(JSON.parse(e.data));
    } catch {
      onMessage(e.data);
    }
  };
  if (onError) ws.onerror = onError;
  return ws;
}
