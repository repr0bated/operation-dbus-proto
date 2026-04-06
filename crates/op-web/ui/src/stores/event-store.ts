import { create } from "zustand";
import type { HealthSnapshot, LogEntry, Agent, EventLogEntry, SseEventType } from "@/types/api";

interface EventStore {
  connected: boolean;
  health: HealthSnapshot | null;
  logs: LogEntry[];
  agents: Agent[];
  events: EventLogEntry[];
  lastError: string | null;
  eventCounts: Record<string, number>;
  latestState: Record<string, unknown>;
  latestStats: Record<string, unknown> | null;

  setConnected: (v: boolean) => void;
  setHealth: (h: HealthSnapshot) => void;
  addLog: (entry: LogEntry) => void;
  setLogs: (logs: LogEntry[]) => void;
  setAgents: (agents: Agent[]) => void;
  addEvent: (event: EventLogEntry) => void;
  setError: (err: string | null) => void;
  clearLogs: () => void;
  updateState: (key: string, value: unknown) => void;
  setStats: (stats: Record<string, unknown>) => void;
  incrementEventCount: (type: SseEventType) => void;
}

const MAX_LOGS = 1000;
const MAX_EVENTS = 500;

export const useEventStore = create<EventStore>((set) => ({
  connected: false,
  health: null,
  logs: [],
  agents: [],
  events: [],
  lastError: null,
  eventCounts: {},
  latestState: {},
  latestStats: null,

  setConnected: (connected) => set({ connected }),
  setHealth: (health) => set({ health }),
  addLog: (entry) => set((s) => ({ logs: [...s.logs, entry].slice(-MAX_LOGS) })),
  setLogs: (logs) => set({ logs }),
  setAgents: (agents) => set({ agents }),
  addEvent: (event) => set((s) => ({ events: [...s.events, event].slice(-MAX_EVENTS) })),
  setError: (lastError) => set({ lastError }),
  clearLogs: () => set({ logs: [] }),
  updateState: (key, value) => set((s) => ({
    latestState: { ...s.latestState, [key]: value },
  })),
  setStats: (stats) => set({ latestStats: stats }),
  incrementEventCount: (type) => set((s) => ({
    eventCounts: { ...s.eventCounts, [type]: (s.eventCounts[type] || 0) + 1 },
  })),
}));
