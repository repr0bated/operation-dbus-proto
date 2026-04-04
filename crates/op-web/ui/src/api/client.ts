// Typed API client for operation-dbus
import type {
  HealthResponse, StatusResponse, ToolInfo, ToolExecutionRequest,
  ToolExecutionResult, LlmStatus, LlmModel, ChatRequest, ChatMessage,
  AgentInfo, ConfigSection, SecurityInfo,
} from "./types";

const API_BASE = import.meta.env.VITE_API_BASE_URL || "";

class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers: { "Content-Type": "application/json", ...options?.headers },
  });
  if (!res.ok) {
    const msg = await res.text().catch(() => res.statusText);
    throw new ApiError(res.status, msg);
  }
  return res.json();
}

export const api = {
  // Health & Status
  getHealth: () => request<HealthResponse>("/api/health"),
  getStatus: () => request<StatusResponse>("/api/status"),

  // Tools
  getTools: () => request<ToolInfo[]>("/api/tools"),
  getTool: (name: string) => request<ToolInfo>(`/api/tools/${encodeURIComponent(name)}`),
  executeTool: (name: string, args: ToolExecutionRequest) =>
    request<ToolExecutionResult>(`/api/tools/${encodeURIComponent(name)}/execute`, {
      method: "POST",
      body: JSON.stringify(args),
    }),

  // LLM
  getLlmStatus: () => request<LlmStatus>("/api/llm/status"),
  getLlmModels: () => request<LlmModel[]>("/api/llm/models"),
  setLlmModel: (model: string) =>
    request<LlmStatus>("/api/llm/model", { method: "POST", body: JSON.stringify({ model }) }),

  // Chat
  sendChat: (req: ChatRequest) =>
    request<ChatMessage>("/api/chat", { method: "POST", body: JSON.stringify(req) }),

  // Agents
  getAgents: () => request<AgentInfo[]>("/api/agents"),
  getAgent: (id: string) => request<AgentInfo>(`/api/agents/${id}`),

  // Config
  getConfig: () => request<ConfigSection[]>("/api/config"),
  updateConfig: (key: string, values: Record<string, unknown>) =>
    request<ConfigSection>(`/api/config/${key}`, { method: "PUT", body: JSON.stringify(values) }),

  // Security
  getSecurity: () => request<SecurityInfo>("/api/security"),
};

export { ApiError };
