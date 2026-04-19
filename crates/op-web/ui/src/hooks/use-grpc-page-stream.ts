/**
 * useGrpcPageStream — gRPC-Web hook for schema-driven page element rendering.
 *
 * Fetches the PluginSchema for a given plugin via PluginService.GetSchema,
 * then subscribes to StateSync.Subscribe for live state changes filtered
 * to that plugin. Projects the 1:1 shared-memory state (The Sled) into
 * a renderable { schema, data } pair that GrpcPageRenderer consumes.
 *
 * Injects X-Ghostbridge-Trace-ID and X-Ghostbridge-Footprint from
 * sessionStorage into all gRPC metadata for the Accountability Loop.
 */

import { useState, useEffect, useRef, useCallback } from "react";
import { pluginService, stateSync } from "@/grpc/client";
import { valueToJs, structToObject } from "@/grpc/google/protobuf/struct";
import type { JsonSchema } from "@/types/api";
import type { StateChange, GetSchemaResponse } from "@/grpc/types/operation";

// ── Types ───────────────────────────────────────────────────────────────────

export interface PageElement {
  /** Plugin ID that owns this element */
  pluginId: string;
  /** JSON Schema describing the element structure */
  schema: JsonSchema;
  /** Current data projected from the live gRPC state stream */
  data: Record<string, unknown>;
  /** Event chain event ID of the last mutation */
  lastEventId: number;
  /** Effective hash from the last state snapshot */
  effectiveHash: string;
}

export interface GrpcPageStreamState {
  /** All page elements keyed by plugin ID */
  elements: Record<string, PageElement>;
  /** Ordered list of element keys for render ordering */
  elementOrder: string[];
  /** Whether the schema has been loaded */
  schemaLoaded: boolean;
  /** Whether the live state stream is connected */
  streamConnected: boolean;
  /** Loading state for initial schema fetch */
  loading: boolean;
  /** Error message if schema fetch or stream fails */
  error: string | null;
  /** Ghostbridge trace metadata bound to this stream */
  traceId: string;
  /** Ghostbridge footprint bound to this stream */
  footprint: string;
  /** Total mutations observed on this stream */
  mutationCount: number;
}

export interface UseGrpcPageStreamOptions {
  /** Plugin IDs to subscribe to. Empty = all plugins. */
  pluginIds?: string[];
  /** D-Bus path patterns to filter (glob). Empty = all. */
  pathPatterns?: string[];
  /** Tags to filter state changes by. */
  tags?: string[];
  /** Schema format to request (default: json-schema-2026). */
  schemaFormat?: string;
  /** Whether to include the initial state snapshot in the stream. */
  includeInitialState?: boolean;
  /** Disable auto-connect (useful for conditional rendering). */
  disabled?: boolean;
}

// ── Hook ────────────────────────────────────────────────────────────────────

export function useGrpcPageStream(options: UseGrpcPageStreamOptions = {}): GrpcPageStreamState {
  const {
    pluginIds = [],
    pathPatterns = [],
    tags = [],
    schemaFormat = "json-schema-2026",
    includeInitialState = true,
    disabled = false,
  } = options;

  const [state, setState] = useState<GrpcPageStreamState>({
    elements: {},
    elementOrder: [],
    schemaLoaded: false,
    streamConnected: false,
    loading: false,
    error: null,
    traceId: "",
    footprint: "",
    mutationCount: 0,
  });

  const abortRef = useRef<(() => void) | null>(null);
  const mountedRef = useRef(true);

  // Read Ghostbridge headers once on mount
  useEffect(() => {
    const traceId = sessionStorage.getItem("X-Ghostbridge-Trace-ID") || "";
    const footprint = sessionStorage.getItem("X-Ghostbridge-Footprint") || "";
    setState((prev) => ({ ...prev, traceId, footprint }));
  }, []);

  // Fetch schemas for all requested plugins
  const fetchSchemas = useCallback(async (ids: string[]): Promise<Record<string, JsonSchema>> => {
    const schemas: Record<string, JsonSchema> = {};

    if (ids.length === 0) {
      // No specific plugins — fetch the plugin list first, then each schema
      try {
        const listResp = await pluginService.listPlugins();
        for (const plugin of listResp.plugins) {
          ids.push(plugin.id);
        }
      } catch {
        // If listing fails, return empty — stream will still project raw state
        return schemas;
      }
    }

    // Fetch each plugin's schema in parallel
    const results = await Promise.allSettled(
      ids.map(async (id) => {
        const resp: GetSchemaResponse = await pluginService.getSchema({
          pluginId: id,
          format: schemaFormat,
        });
        return { id, schema: JSON.parse(resp.schemaJson) as JsonSchema };
      }),
    );

    for (const result of results) {
      if (result.status === "fulfilled") {
        schemas[result.value.id] = result.value.schema;
      }
    }

    return schemas;
  }, [schemaFormat]);

  // Main effect: fetch schemas then subscribe to state stream
  useEffect(() => {
    if (disabled) return;
    mountedRef.current = true;

    let streamAbort: (() => void) | null = null;

    async function connect() {
      if (!mountedRef.current) return;

      setState((prev) => ({ ...prev, loading: true, error: null }));

      // ── Step 1: Fetch PluginSchema for each plugin via gRPC ──
      let schemas: Record<string, JsonSchema> = {};
      try {
        schemas = await fetchSchemas([...pluginIds]);
      } catch (err) {
        if (mountedRef.current) {
          setState((prev) => ({
            ...prev,
            loading: false,
            error: `Schema fetch failed: ${(err as Error).message}`,
          }));
        }
        return;
      }

      if (!mountedRef.current) return;

      // Initialize elements from fetched schemas
      const elements: Record<string, PageElement> = {};
      const elementOrder: string[] = [];

      for (const [pluginId, schema] of Object.entries(schemas)) {
        elements[pluginId] = {
          pluginId,
          schema,
          data: {},
          lastEventId: 0,
          effectiveHash: "",
        };
        elementOrder.push(pluginId);
      }

      setState((prev) => ({
        ...prev,
        elements,
        elementOrder,
        schemaLoaded: Object.keys(schemas).length > 0,
        loading: false,
      }));

      // ── Step 2: Subscribe to StateSync stream for live updates ──
      try {
        const { stream, abort } = stateSync.subscribe({
          pluginIds,
          pathPatterns,
          tags,
          includeInitialState,
        });
        streamAbort = abort;
        abortRef.current = abort;

        const reader = stream.getReader();

        // Mark stream as connected
        if (mountedRef.current) {
          setState((prev) => ({ ...prev, streamConnected: true, error: null }));
        }

        while (true) {
          const { done, value } = await reader.read();
          if (done || !mountedRef.current) break;

          const change = value as StateChange;
          const pluginId = change.pluginId || "__global__";
          const memberName = change.memberName;
          const newValue = valueToJs(change.newValue);

          setState((prev) => {
            const existing = prev.elements[pluginId];
            const updatedData = existing
              ? { ...existing.data, [memberName]: newValue }
              : { [memberName]: newValue };

            // If this plugin had no schema, create a stub element
            const updatedElement: PageElement = existing
              ? {
                  ...existing,
                  data: updatedData,
                  lastEventId: change.eventId,
                  effectiveHash: change.eventHash,
                }
              : {
                  pluginId,
                  schema: { type: "object", properties: {} },
                  data: updatedData,
                  lastEventId: change.eventId,
                  effectiveHash: change.eventHash,
                };

            // Dynamically extend schema properties for new members
            if (updatedElement.schema.properties && !updatedElement.schema.properties[memberName]) {
              updatedElement.schema = {
                ...updatedElement.schema,
                properties: {
                  ...updatedElement.schema.properties,
                  [memberName]: inferSchemaType(newValue),
                },
              };
            }

            const newOrder = prev.elementOrder.includes(pluginId)
              ? prev.elementOrder
              : [...prev.elementOrder, pluginId];

            return {
              ...prev,
              elements: { ...prev.elements, [pluginId]: updatedElement },
              elementOrder: newOrder,
              mutationCount: prev.mutationCount + 1,
            };
          });
        }
      } catch (err) {
        if (mountedRef.current && (err as Error).name !== "AbortError") {
          setState((prev) => ({
            ...prev,
            streamConnected: false,
            error: `State stream error: ${(err as Error).message}`,
          }));
        }
      }
    }

    connect();

    return () => {
      mountedRef.current = false;
      if (streamAbort) {
        try { streamAbort(); } catch { /* ignore */ }
      }
      abortRef.current = null;
    };
  // Stable deps — options are primitives/arrays compared by ref
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [disabled, fetchSchemas]);

  return state;
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/** Infer a minimal JSON Schema type from a JS runtime value */
function inferSchemaType(value: unknown): JsonSchema {
  if (value === null || value === undefined) {
    return { type: "string" };
  }
  if (typeof value === "boolean") {
    return { type: "boolean" };
  }
  if (typeof value === "number") {
    return Number.isInteger(value) ? { type: "integer" } : { type: "number" };
  }
  if (typeof value === "string") {
    return { type: "string" };
  }
  if (Array.isArray(value)) {
    return { type: "array", items: { type: "string" } };
  }
  if (typeof value === "object") {
    return { type: "object", properties: {} };
  }
  return { type: "string" };
}
