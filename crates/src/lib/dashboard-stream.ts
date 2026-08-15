export type DashboardEventType =
  | "state_update"
  | "schema_update"
  | "audit_event"
  | "system_stats"
  | "message"
  | "unknown";

const knownDashboardEventTypes = [
  "state_update",
  "schema_update",
  "audit_event",
  "system_stats",
  "message",
] as const;

type KnownDashboardEventType = (typeof knownDashboardEventTypes)[number];

/**
 * A state frame, forwarded whole from the gRPC stream. The field list mirrors
 * the wire frame rather than what any one component happens to render, so a new
 * consumer never needs a change at the transport hop to see something.
 */
export interface StateUpdatePayload {
  change_id: string;
  plugin_id: string;
  object_path: string;
  property_name: string | null;
  old_value: unknown;
  new_value: unknown;
  event_id: string;
  event_hash: string;
  tags_touched: string[];
  actor_id: string;
  timestamp: string | null;
  /** property_set, method_call, signal, schema_migration, … */
  change_type: string;
  /** initial_state (hydration), update (live), heartbeat (keepalive). */
  frame_kind: string;
  /**
   * Contract this frame's plugin is published under. Null only on frames that
   * name no plugin. Compare it against the contract held for that plugin: a
   * mismatch means a schema frame was missed.
   */
  schema_hash: string | null;
  /**
   * Identity of the whole published catalog, present on every frame including
   * keepalives — so an idle stream can still reveal that contracts moved.
   */
  catalog_hash: string;
}

/**
 * One plugin's sealed contract. Same frame shape as a state update — it arrives
 * on the same stream — distinguished by change_type "schema_migration". The
 * contract itself rides in `new_value` exactly as it was sealed into the blob
 * catalog, so the UI never reads the catalog; `schema_hash` says which one.
 */
export type SchemaUpdatePayload = StateUpdatePayload;

/** The contract carried by a schema frame. */
export function schemaFromFrame(payload: SchemaUpdatePayload): unknown {
  return payload.new_value;
}

/** A chain frame, forwarded whole. The hashes are what make it verifiable. */
export interface AuditEventPayload {
  event_id: string;
  event_hash: string;
  prev_hash: string;
  plugin_id: string;
  operation_type: string;
  target: string;
  decision: string;
  tags_touched: string[];
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
  | SchemaUpdatePayload
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
  /** Running contract per plugin id, hydrated at stream open. */
  schemasByPlugin: Record<string, SchemaUpdatePayload>;
  /** Catalog identity as of the last frame received, from any frame type. */
  catalogHash: string;
  /**
   * Plugins whose frames cite a contract we are not holding — the visible
   * symptom of a dropped schema frame. A consumer should re-hydrate these
   * rather than render values against a contract it knows is wrong.
   */
  staleSchemas: string[];
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
    schemasByPlugin: {},
    catalogHash: "",
    staleSchemas: [],
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

function isKnownDashboardEventType(
  type: string,
): type is KnownDashboardEventType {
  return (knownDashboardEventTypes as readonly string[]).includes(type);
}

export function parseDashboardStreamEvent(
  type: string,
  raw: string,
): DashboardStreamEvent {
  try {
    const parsed = JSON.parse(raw) as DashboardEventPayload;
    const normalizedType = isKnownDashboardEventType(type) ? type : "unknown";

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
  return `${payload.plugin_id}:${payload.object_path}:${payload.property_name ?? ""}`;
}

/**
 * Does this frame reference a contract we are not holding? That is the only
 * observable trace of a dropped schema frame, since the stream never resends.
 */
function citesUnknownContract(
  state: DashboardStreamState,
  payload: StateUpdatePayload,
): boolean {
  if (!payload.plugin_id || !payload.schema_hash) return false;
  const held = state.schemasByPlugin[payload.plugin_id];
  return held !== undefined && held.schema_hash !== payload.schema_hash;
}

function withStale(current: string[], pluginId: string): string[] {
  return current.includes(pluginId) ? current : [...current, pluginId];
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
    const catalogHash = payload.catalog_hash || state.catalogHash;
    // Keepalives carry no plugin and no value; counting them is the point of
    // them, keying them would invent an empty plugin entry. They still carry
    // the catalog hash, which is what makes an idle stream informative.
    if (payload.frame_kind === "heartbeat") {
      return { ...state, events: nextEvents, counters, catalogHash };
    }
    return {
      ...state,
      events: nextEvents,
      counters,
      catalogHash,
      staleSchemas: citesUnknownContract(state, payload)
        ? withStale(state.staleSchemas, payload.plugin_id)
        : state.staleSchemas,
      latestStateByKey: {
        ...state.latestStateByKey,
        [stateUpdateKey(payload)]: payload,
      },
    };
  }

  if (event.type === "schema_update") {
    const payload = event.payload as SchemaUpdatePayload;
    return {
      ...state,
      events: nextEvents,
      counters,
      catalogHash: payload.catalog_hash || state.catalogHash,
      // Receiving the contract is what clears the staleness it caused.
      staleSchemas: state.staleSchemas.filter((id) => id !== payload.plugin_id),
      schemasByPlugin: {
        ...state.schemasByPlugin,
        [payload.plugin_id]: payload,
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
