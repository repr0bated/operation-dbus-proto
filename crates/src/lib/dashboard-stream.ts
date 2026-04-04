export type DashboardEventType =
  | "state_update"
  | "audit_event"
  | "system_stats"
  | "message"
  | "unknown";

export interface StateUpdatePayload {
  plugin_id: string;
  object_path: string;
  property_name: string;
  new_value: unknown;
  event_id?: string;
  tags?: string[];
}

export interface AuditEventPayload {
  event_id: string;
  plugin_id: string;
  operation: string;
  target: string;
  decision?: string;
  tags?: string[];
}

export interface SystemStatsPayload {
  uptime_secs: number;
  memory_total_mb: number;
  memory_used_mb: number;
  cpu_usage: number;
}

export interface UnknownEventPayload {
  raw: string;
}

export type DashboardEventPayload =
  | StateUpdatePayload
  | AuditEventPayload
  | SystemStatsPayload
  | UnknownEventPayload;

export interface DashboardStreamEvent {
  type: DashboardEventType;
  receivedAt: number;
  payload: DashboardEventPayload;
}

export interface DashboardStreamState {
  connected: boolean;
  events: DashboardStreamEvent[];
  counters: Record<string, number>;
  latestStateByKey: Record<string, StateUpdatePayload>;
  latestSystemStats: SystemStatsPayload | null;
  lastAuditEvent: AuditEventPayload | null;
  parseErrors: number;
}

export const MAX_STREAM_EVENTS = 40;

export function createInitialDashboardStreamState(): DashboardStreamState {
  return {
    connected: false,
    events: [],
    counters: {},
    latestStateByKey: {},
    latestSystemStats: null,
    lastAuditEvent: null,
    parseErrors: 0,
  };
}

function parseUnknownEvent(type: string, raw: string): DashboardStreamEvent {
  return {
    type: type === "message" ? "message" : "unknown",
    receivedAt: Date.now(),
    payload: { raw },
  };
}

export function parseDashboardStreamEvent(
  type: string,
  raw: string,
): DashboardStreamEvent {
  try {
    const parsed = JSON.parse(raw) as DashboardEventPayload;
    const normalizedType = ([
      "state_update",
      "audit_event",
      "system_stats",
      "message",
    ] as const).includes(type as DashboardEventType)
      ? (type as DashboardEventType)
      : "unknown";

    return {
      type: normalizedType,
      receivedAt: Date.now(),
      payload: parsed,
    };
  } catch {
    return parseUnknownEvent(type, raw);
  }
}

function stateUpdateKey(payload: StateUpdatePayload): string {
  return `${payload.plugin_id}:${payload.object_path}:${payload.property_name}`;
}

export function reduceDashboardStreamEvent(
  state: DashboardStreamState,
  event: DashboardStreamEvent,
): DashboardStreamState {
  const nextEvents = [event, ...state.events].slice(0, MAX_STREAM_EVENTS);
  const counters = {
    ...state.counters,
    [event.type]: (state.counters[event.type] ?? 0) + 1,
  };

  if (event.type === "state_update") {
    const payload = event.payload as StateUpdatePayload;
    return {
      ...state,
      events: nextEvents,
      counters,
      latestStateByKey: {
        ...state.latestStateByKey,
        [stateUpdateKey(payload)]: payload,
      },
    };
  }

  if (event.type === "audit_event") {
    return {
      ...state,
      events: nextEvents,
      counters,
      lastAuditEvent: event.payload as AuditEventPayload,
    };
  }

  if (event.type === "system_stats") {
    return {
      ...state,
      events: nextEvents,
      counters,
      latestSystemStats: event.payload as SystemStatsPayload,
    };
  }

  return {
    ...state,
    events: nextEvents,
    counters,
    parseErrors:
      "raw" in event.payload ? state.parseErrors + 1 : state.parseErrors,
  };
}
