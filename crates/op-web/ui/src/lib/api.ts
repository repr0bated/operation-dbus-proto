/**
 * Typed API client for the operation-dbus gateway.
 * Unary RPCs use gRPC-Web; streaming is handled by use-event-stream.
 * Falls back to REST for endpoints not yet ported to gRPC.
 */
import {
  stateSync,
  pluginService,
  ovsdbMirror,
  runtimeMirror,
  eventChainService,
  componentRegistry,
} from "@/grpc/client";
import { structToObject } from "@/grpc/google/protobuf/struct";
import type {
  HealthSnapshot, StatusSummary, DbusService, Tool, Agent, Session,
  ChatMessage, LogEntry, ConfigSnapshot, LlmProvider, LlmModel,
} from "@/types/api";

const BASE = import.meta.env.VITE_API_BASE_URL || "/api";

/** REST fallback for endpoints not yet gRPC-ified */
async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!res.ok) throw new Error(`API ${res.status}: ${await res.text()}`);
  return res.json();
}

export const api = {
  // ── gRPC-powered endpoints ──────────────────────────────────────────────

  /** Get full state snapshot via gRPC StateSync.GetState */
  state: async (pluginId = "", objectPath = "") => {
    const resp = await stateSync.getState({ pluginId, objectPath });
    return structToObject(resp.state);
  },

  /** Mutate state via gRPC StateSync.Mutate */
  mutate: stateSync.mutate,
  batchMutate: stateSync.batchMutate,

  /** Plugin operations via gRPC PluginService */
  plugins: {
    list: async () => {
      const resp = await pluginService.listPlugins();
      return resp.plugins;
    },
    getSchema: pluginService.getSchema,
    callMethod: pluginService.callMethod,
  },

  /** Event chain / audit via gRPC EventChainService */
  eventChain: {
    getEvents: eventChainService.getEvents,
    verifyChain: eventChainService.verifyChain,
  },

  /** OVSDB via gRPC OvsdbMirror */
  ovs: {
    listDbs: ovsdbMirror.listDbs,
    getSchema: ovsdbMirror.getSchema,
    transact: ovsdbMirror.transact,
    getBridgeState: ovsdbMirror.getBridgeState,
  },

  /** Runtime via gRPC RuntimeMirror */
  runtime: {
    getSystemInfo: runtimeMirror.getSystemInfo,
    listServices: runtimeMirror.listServices,
    getService: runtimeMirror.getService,
    listInterfaces: runtimeMirror.listInterfaces,
    getNumaTopology: runtimeMirror.getNumaTopology,
  },

  /** Component registry via gRPC */
  registry: {
    discover: componentRegistry.discover,
    getComponent: componentRegistry.getComponent,
  },

  // ── REST fallbacks (not yet ported to gRPC) ─────────────────────────────

  health: () => request<HealthSnapshot>("/health"),
  status: () => request<StatusSummary>("/status"),

  services: {
    list: () => request<DbusService[]>("/services"),
    get: (name: string) => request<DbusService>(`/services/${encodeURIComponent(name)}`),
    introspect: (name: string, path: string) =>
      request<unknown>(`/services/${encodeURIComponent(name)}/introspect?path=${encodeURIComponent(path)}`),
    call: (name: string, path: string, iface: string, method: string, args: unknown[] = []) =>
      request<unknown>(`/services/${encodeURIComponent(name)}/call`, {
        method: "POST", body: JSON.stringify({ path, interface: iface, method, args }),
      }),
  },

  tools: {
    list: () => request<Tool[]>("/tools"),
    get: (id: string) => request<Tool>(`/tools/${encodeURIComponent(id)}`),
    execute: (id: string, input: Record<string, unknown>) =>
      request<unknown>(`/tools/${encodeURIComponent(id)}/execute`, {
        method: "POST", body: JSON.stringify(input),
      }),
  },

  agents: {
    list: () => request<Agent[]>("/agents"),
    get: (id: string) => request<Agent>(`/agents/${id}`),
    start: (id: string) => request<void>(`/agents/${id}/start`, { method: "POST" }),
    stop: (id: string) => request<void>(`/agents/${id}/stop`, { method: "POST" }),
  },

  llm: {
    status: () => request<LlmProvider[]>("/llm/status"),
    models: () => request<LlmModel[]>("/llm/models"),
    setModel: (modelId: string) => request<void>("/llm/model", {
      method: "POST", body: JSON.stringify({ modelId }),
    }),
  },

  sessions: {
    list: () => request<Session[]>("/sessions"),
    get: (key: string) => request<Session>(`/sessions/${encodeURIComponent(key)}`),
    delete: (key: string) => request<void>(`/sessions/${encodeURIComponent(key)}`, { method: "DELETE" }),
    patch: (key: string, patch: Record<string, unknown>) =>
      request<void>(`/sessions/${encodeURIComponent(key)}`, {
        method: "PATCH", body: JSON.stringify(patch),
      }),
  },

  chat: {
    history: (sessionKey: string) =>
      request<ChatMessage[]>(`/chat/${encodeURIComponent(sessionKey)}/history`),
    send: (sessionKey: string, message: string, attachments?: unknown[]) =>
      request<ChatMessage>(`/chat`, {
        method: "POST", body: JSON.stringify({ sessionKey, message, attachments }),
      }),
  },

  logs: {
    list: (opts?: { limit?: number; level?: string }) => {
      const p = new URLSearchParams();
      if (opts?.limit) p.set("limit", String(opts.limit));
      if (opts?.level) p.set("level", opts.level);
      return request<LogEntry[]>(`/logs?${p}`);
    },
  },

  config: {
    get: () => request<ConfigSnapshot>("/config"),
    save: (config: Record<string, unknown>) =>
      request<void>("/config", { method: "PUT", body: JSON.stringify(config) }),
    apply: () => request<void>("/config/apply", { method: "POST" }),
  },

  debug: {
    call: (method: string, params: unknown) =>
      request<unknown>("/debug/call", {
        method: "POST", body: JSON.stringify({ method, params }),
      }),
  },
};

/**
 * @deprecated Use `useEventStream()` hook instead — SSE has been replaced by gRPC-Web streams.
 */
export function connectEventStream(
  onEvent: (event: { type: string; data: unknown }) => void,
  onError: (err: Error) => void,
): () => void {
  console.warn("[DEPRECATED] connectEventStream: SSE has been replaced by gRPC-Web. Use useEventStream() hook.");
  // No-op — streams are now managed by useEventStream
  return () => {};
}
