// Centralized SSE event stream store
import { create } from "zustand";
import type { SSEEvent, StateEntry } from "./types";

interface EventStreamState {
  connected: boolean;
  events: SSEEvent[];
  counters: Record<string, number>;
  latestStats: Record<string, unknown> | null;
  latestState: Map<string, StateEntry>;
  lastAuditEvent: SSEEvent | null;
  parseErrors: number;
  eventSource: EventSource | null;
  // Actions
  connect: () => void;
  disconnect: () => void;
  addEvent: (event: SSEEvent) => void;
}

const MAX_EVENTS = 500;

export const useEventStream = create<EventStreamState>((set, get) => ({
  connected: false,
  events: [],
  counters: {},
  latestStats: null,
  latestState: new Map(),
  lastAuditEvent: null,
  parseErrors: 0,
  eventSource: null,

  connect: () => {
    const existing = get().eventSource;
    if (existing) existing.close();

    const base = import.meta.env.VITE_API_BASE_URL || "";
    const es = new EventSource(`${base}/api/events`);

    es.onopen = () => set({ connected: true });
    es.onerror = () => set({ connected: false });

    es.onmessage = (e) => {
      try {
        const parsed = JSON.parse(e.data);
        const event: SSEEvent = {
          id: parsed.id || crypto.randomUUID(),
          type: parsed.type || "unknown",
          data: parsed.data ?? parsed,
          timestamp: parsed.timestamp || new Date().toISOString(),
        };
        get().addEvent(event);
      } catch {
        set((s) => ({ parseErrors: s.parseErrors + 1 }));
      }
    };

    // Handle named events
    for (const type of ["state_update", "audit_event", "system_stats", "message"]) {
      es.addEventListener(type, (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data);
          const event: SSEEvent = {
            id: crypto.randomUUID(),
            type,
            data,
            timestamp: new Date().toISOString(),
          };
          get().addEvent(event);
        } catch {
          set((s) => ({ parseErrors: s.parseErrors + 1 }));
        }
      });
    }

    set({ eventSource: es, connected: true });
  },

  disconnect: () => {
    const es = get().eventSource;
    if (es) es.close();
    set({ eventSource: null, connected: false });
  },

  addEvent: (event) =>
    set((state) => {
      const events = [event, ...state.events].slice(0, MAX_EVENTS);
      const counters = { ...state.counters, [event.type]: (state.counters[event.type] || 0) + 1 };

      let latestStats = state.latestStats;
      let latestState = state.latestState;
      let lastAuditEvent = state.lastAuditEvent;

      if (event.type === "system_stats") {
        latestStats = event.data as Record<string, unknown>;
      }
      if (event.type === "state_update" && typeof event.data === "object" && event.data !== null) {
        const d = event.data as Record<string, unknown>;
        const key = (d.key as string) || "";
        if (key) {
          latestState = new Map(latestState);
          latestState.set(key, {
            key,
            plugin: (d.plugin as string) || "",
            object_type: (d.object_type as string) || "",
            value: d.value,
            updated_at: event.timestamp,
          });
        }
      }
      if (event.type === "audit_event") {
        lastAuditEvent = event;
      }

      return { events, counters, latestStats, latestState, lastAuditEvent };
    }),
}));
