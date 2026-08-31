/**
 * Dynamic page spec hook.
 *
 * Returns a json-render Spec for a route:
 *   1. If route is in PAGE_SPECS, return the static spec immediately.
 *   2. Otherwise, find which plugin owns this route via its ui_surfaces.
 *   3. Fetch the plugin schema and generate a spec from ui_projection.
 *   4. Memoize by (route, schemaHash) to avoid re-fetching on navigation.
 *   5. Return emptyState spec if no plugin claims the route.
 */
import { useState, useEffect, useMemo, useRef } from "react";
import type { Spec } from "@json-render/core";
import { PAGE_SPECS } from "@/json-render/pages";
import { useEventStore } from "@/stores/event-store";
import { useBlobCatalog } from "@/hooks/use-blob-catalog";
import { resolveApiBase } from "@/lib/api";
import { generatePluginPageSpec } from "./generate-plugin-page";
import type { UiSubidProjection, UiSurfaceProjection } from "@/lib/subid-ui";
import { isAuthoritative } from "@/lib/subid-ui";

export interface UsePluginPageSpecResult {
  spec: Spec | null;
  loading: boolean;
  error: string | null;
  isStatic: boolean;
}

interface PluginSchemaResponse {
  plugin: string;
  schema_hash?: string;
  schema?: {
    display_name?: string;
    fields?: {
      ui_surfaces?: {
        default?: unknown;
      };
    };
  };
  ui_projection?: UiSubidProjection[];
  ui_surfaces?: UiSurfaceProjection;
}

/** Cache for fetched specs, keyed by plugin + schemaHash. */
const specCache = new Map<string, Spec>();

/**
 * Extract display name from schema or fall back to plugin id.
 */
function getDisplayName(response: PluginSchemaResponse): string {
  return response.schema?.display_name ?? response.plugin;
}

/**
 * Parse ui_surfaces routes from a plugin schema response.
 */
function parseUiSurfaceRoutes(response: PluginSchemaResponse): string[] {
  // Prefer the pre-parsed ui_surfaces projection
  if (response.ui_surfaces && isAuthoritative(response.ui_surfaces)) {
    return response.ui_surfaces.routes.map((r) => r.path);
  }
  // Fall back to parsing from schema fields
  const raw = response.schema?.fields?.ui_surfaces?.default;
  if (!Array.isArray(raw)) return [];
  return raw
    .filter(
      (item): item is { path: string } =>
        item && typeof item === "object" && typeof (item as { path?: unknown }).path === "string",
    )
    .map((item) => item.path);
}

/**
 * Fetch plugin schema from the API.
 */
async function fetchPluginSchema(pluginId: string): Promise<PluginSchemaResponse> {
  const response = await fetch(
    `${resolveApiBase()}/ui-model/plugin-schema/${encodeURIComponent(pluginId)}`,
  );
  if (!response.ok) {
    throw new Error(`Plugin schema fetch failed: HTTP ${response.status}`);
  }
  return response.json() as Promise<PluginSchemaResponse>;
}

/**
 * Find which plugin owns a route by checking ui_surfaces.
 * Returns the plugin id and its schema response, or null if no plugin owns the route.
 */
async function findPluginForRoute(
  route: string,
  pluginIds: string[],
  schemas: Record<string, unknown>,
): Promise<{ pluginId: string; response: PluginSchemaResponse } | null> {
  // First check schemas already in the store
  for (const pluginId of pluginIds) {
    const schema = schemas[pluginId] as PluginSchemaResponse | undefined;
    if (schema) {
      const routes = parseUiSurfaceRoutes(schema);
      if (routes.includes(route)) {
        return { pluginId, response: schema };
      }
    }
  }

  // Fetch schemas for plugins not yet in store
  const fetchPromises = pluginIds
    .filter((id) => !schemas[id])
    .map(async (pluginId) => {
      try {
        const response = await fetchPluginSchema(pluginId);
        const routes = parseUiSurfaceRoutes(response);
        if (routes.includes(route)) {
          return { pluginId, response };
        }
      } catch {
        // Skip plugins that fail to fetch
      }
      return null;
    });

  const results = await Promise.all(fetchPromises);
  return results.find((r): r is NonNullable<typeof r> => r !== null) ?? null;
}

/**
 * Generate an empty state spec for routes with no associated page.
 */
function emptyStateSpec(route: string): Spec {
  return {
    root: "root",
    elements: {
      root: {
        type: "emptyState",
        props: {
          title: "Page Not Found",
          hint: `No page spec or plugin ui_surface claims route: ${route}`,
        },
      },
    },
  };
}

/**
 * Generate a loading spec while fetching plugin data.
 */
function loadingSpec(route: string): Spec {
  return {
    root: "root",
    elements: {
      root: {
        type: "emptyState",
        props: {
          title: "Loading...",
          hint: `Resolving page for ${route}`,
        },
      },
    },
  };
}

/**
 * Hook that returns a Spec for a route.
 *
 * - Static routes from PAGE_SPECS are returned immediately.
 * - Dynamic routes are resolved by finding the plugin that owns the route.
 * - Specs are memoized by route + schemaHash to avoid refetching.
 */
export function usePluginPageSpec(route: string): UsePluginPageSpecResult {
  const { plugins } = useBlobCatalog();
  const schemas = useEventStore((s) => s.schemas);
  const schemaHashes = useEventStore((s) => s.schemaHashes);

  // Check static specs first
  const staticSpec = PAGE_SPECS[route];
  if (staticSpec) {
    return {
      spec: staticSpec,
      loading: false,
      error: null,
      isStatic: true,
    };
  }

  const [spec, setSpec] = useState<Spec | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Track the current fetch to avoid race conditions
  const fetchIdRef = useRef(0);

  // Compute a stable cache key from schema hashes
  const cacheKey = useMemo(() => {
    const hashList = plugins
      .map((id) => schemaHashes[id] ?? "")
      .filter(Boolean)
      .sort()
      .join(",");
    return `${route}:${hashList}`;
  }, [route, plugins, schemaHashes]);

  useEffect(() => {
    let cancelled = false;
    const fetchId = ++fetchIdRef.current;

    async function resolveSpec() {
      // Check cache first
      const cached = specCache.get(cacheKey);
      if (cached) {
        setSpec(cached);
        setLoading(false);
        setError(null);
        return;
      }

      setLoading(true);
      setError(null);

      try {
        const result = await findPluginForRoute(route, plugins, schemas);

        if (cancelled || fetchId !== fetchIdRef.current) return;

        if (!result) {
          // No plugin claims this route
          const emptySpec = emptyStateSpec(route);
          setSpec(emptySpec);
          setLoading(false);
          return;
        }

        const { pluginId, response } = result;
        const projections = response.ui_projection ?? [];
        const displayName = getDisplayName(response);

        const generatedSpec = generatePluginPageSpec(pluginId, displayName, projections);

        // Cache the spec
        specCache.set(cacheKey, generatedSpec);

        if (!cancelled && fetchId === fetchIdRef.current) {
          setSpec(generatedSpec);
          setLoading(false);
        }
      } catch (err) {
        if (!cancelled && fetchId === fetchIdRef.current) {
          setError(err instanceof Error ? err.message : String(err));
          setLoading(false);
        }
      }
    }

    resolveSpec();

    return () => {
      cancelled = true;
    };
  }, [route, plugins, schemas, cacheKey]);

  return {
    spec,
    loading,
    error,
    isStatic: false,
  };
}
