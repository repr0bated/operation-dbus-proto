/**
 * Dynamic navigation hook.
 *
 * Enriches NAV_MANIFEST with entries from plugin ui_surfaces:
 *   1. Start with NAV_MANIFEST (static routes).
 *   2. For each plugin in useBlobCatalog().plugins, check for ui_surfaces in schemas.
 *   3. For each authoritative surface route, add a NavItem to a "Plugins" section.
 *   4. Deduplicate: if route already in NAV_MANIFEST, static wins.
 *   5. Return merged list.
 */
import { useMemo } from "react";
import { NAV_MANIFEST, type NavItem, type NavSection } from "./manifest";
import { useBlobCatalog } from "@/hooks/use-blob-catalog";
import { useEventStore } from "@/stores/event-store";
import type { UiSurfaceProjection, UiSurfaceRoute } from "@/lib/subid-ui";
import { isAuthoritative } from "@/lib/subid-ui";

/** Section name for plugin-contributed nav entries. */
const PLUGINS_SECTION: NavSection = "Plugins";

/** Starting order value for plugin nav entries. */
const PLUGIN_NAV_ORDER_BASE = 100;

/**
 * Schema shape that may contain ui_surfaces.
 */
interface SchemaWithUiSurfaces {
  display_name?: string;
  fields?: {
    ui_surfaces?: {
      default?: unknown;
    };
  };
  ui_surfaces?: UiSurfaceProjection;
}

/**
 * Extract ui_surfaces routes from a plugin schema.
 */
function extractUiSurfaceRoutes(
  pluginId: string,
  schema: unknown,
): Array<{ route: UiSurfaceRoute; pluginId: string }> {
  const schemaObj = schema as SchemaWithUiSurfaces | null;
  if (!schemaObj) return [];

  // Prefer the pre-parsed ui_surfaces projection
  if (schemaObj.ui_surfaces && isAuthoritative(schemaObj.ui_surfaces)) {
    return schemaObj.ui_surfaces.routes.map((route) => ({ route, pluginId }));
  }

  // Fall back to parsing from schema fields
  const raw = schemaObj.fields?.ui_surfaces?.default;
  if (!Array.isArray(raw)) return [];

  return raw
    .filter(
      (item): item is UiSurfaceRoute =>
        item &&
        typeof item === "object" &&
        typeof (item as UiSurfaceRoute).path === "string",
    )
    .map((route) => ({ route, pluginId }));
}

/**
 * Generate a display label for a nav item.
 * Uses route name if available, otherwise derives from path.
 */
function generateNavLabel(route: UiSurfaceRoute, pluginId: string): string {
  if (route.name) return route.name;

  // Derive from path: /plugin-name/page -> Page
  const parts = route.path.split("/").filter(Boolean);
  if (parts.length > 0) {
    const last = parts[parts.length - 1];
    return last.charAt(0).toUpperCase() + last.slice(1).replace(/-/g, " ");
  }

  return pluginId;
}

/**
 * Generate a nav item ID from plugin and route.
 */
function generateNavId(pluginId: string, route: UiSurfaceRoute): string {
  const pathPart = route.path.replace(/^\//, "").replace(/\//g, "-") || "root";
  return `plugin-${pluginId}-${pathPart}`;
}

/**
 * Create a NavItem from a plugin ui_surface route.
 */
function createPluginNavItem(
  pluginId: string,
  route: UiSurfaceRoute,
  order: number,
): NavItem {
  return {
    id: generateNavId(pluginId, route),
    label: generateNavLabel(route, pluginId),
    route: route.path,
    icon: "Package", // Default icon for plugin routes
    section: PLUGINS_SECTION as typeof SECTION_ORDER[number],
    order,
  };
}

/**
 * Hook that returns NavItem[] with dynamic plugin routes merged.
 *
 * - Static NAV_MANIFEST entries take precedence over plugin routes.
 * - Plugin ui_surfaces routes are added to a "Plugins" section.
 * - Deduplication ensures no route appears twice.
 */
export function useDynamicNav(): NavItem[] {
  const { plugins } = useBlobCatalog();
  const schemas = useEventStore((s) => s.schemas);

  return useMemo(() => {
    // Collect static routes for deduplication
    const staticRoutes = new Set(NAV_MANIFEST.map((item) => item.route));

    // Also include aliases in the static routes set
    for (const item of NAV_MANIFEST) {
      if (item.aliases) {
        for (const alias of item.aliases) {
          staticRoutes.add(alias);
        }
      }
    }

    // Collect plugin routes
    const pluginNavItems: NavItem[] = [];
    let orderCounter = PLUGIN_NAV_ORDER_BASE;

    for (const pluginId of plugins) {
      const schema = schemas[pluginId];
      if (!schema) continue;

      const surfaceRoutes = extractUiSurfaceRoutes(pluginId, schema);

      for (const { route } of surfaceRoutes) {
        // Skip if this route is already claimed by static nav
        if (staticRoutes.has(route.path)) continue;

        // Skip if we've already added this route from another plugin
        if (pluginNavItems.some((item) => item.route === route.path)) continue;

        pluginNavItems.push(createPluginNavItem(pluginId, route, orderCounter++));
      }
    }

    // Merge: static manifest first, then plugin items
    return [...NAV_MANIFEST, ...pluginNavItems];
  }, [plugins, schemas]);
}

/**
 * Check if the Plugins section should be visible.
 * Returns true if there are any plugin-contributed nav items.
 */
export function useHasPluginNav(): boolean {
  const navItems = useDynamicNav();
  return navItems.some((item) => item.section === PLUGINS_SECTION);
}
