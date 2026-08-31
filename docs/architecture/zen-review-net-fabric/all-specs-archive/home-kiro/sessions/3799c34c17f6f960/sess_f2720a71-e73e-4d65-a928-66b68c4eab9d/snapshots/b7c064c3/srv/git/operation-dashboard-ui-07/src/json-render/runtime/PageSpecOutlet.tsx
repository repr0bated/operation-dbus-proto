/**
 * Route-aware page outlet.
 *
 * Uses the usePluginPageSpec hook to resolve specs:
 *   1. Static PAGE_SPECS win.
 *   2. `/plugins/:id` (not `/plugins` itself) renders the sealed-schema ui_projection.
 *   3. Dynamic plugin routes are resolved from ui_surfaces.
 *   4. Everything else falls through to <Outlet />.
 *
 * Shows a loading state while fetching dynamic specs. Shell chrome remains
 * visible; only the content region shows loading.
 */
import { useMemo } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { Renderer } from "@json-render/react";
import { useJsonRender } from "./JsonRenderProvider";
import { PAGE_SPECS } from "@/json-render/pages";
import { ProjectedPluginPage } from "./ProjectedPluginPage";
import { usePluginPageSpec } from "@/json-render/spec-gen";

export { PAGE_SPECS };

function pluginIdFromPath(pathname: string): string | null {
  const match = pathname.match(/^\/plugins\/([^/]+)\/?$/);
  if (!match) return null;
  try {
    return decodeURIComponent(match[1]);
  } catch {
    return match[1];
  }
}

/**
 * Loading spec for the content region.
 */
function loadingSpec(route: string) {
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
 * Error spec for the content region.
 */
function errorSpec(error: string) {
  return {
    root: "root",
    elements: {
      root: {
        type: "statusBanner",
        props: {
          title: "Error",
          message: error,
          tone: "danger",
        },
      },
    },
  };
}

export function PageSpecOutlet() {
  const { pathname } = useLocation();
  const { registry } = useJsonRender();

  // Use the dynamic hook for spec resolution
  const { spec, loading, error, isStatic } = usePluginPageSpec(pathname);

  // Special case: /plugins/:id routes use ProjectedPluginPage
  const pluginId = useMemo(() => {
    // Only check for plugin ID if this isn't a static route
    if (isStatic) return null;
    return pluginIdFromPath(pathname);
  }, [pathname, isStatic]);

  // Static spec from PAGE_SPECS
  if (isStatic && spec) {
    return <Renderer spec={spec} registry={registry} />;
  }

  // /plugins/:id routes use ProjectedPluginPage directly
  if (pluginId) {
    return <ProjectedPluginPage pluginId={pluginId} />;
  }

  // Show loading state while fetching dynamic spec
  if (loading) {
    return <Renderer spec={loadingSpec(pathname)} registry={registry} />;
  }

  // Show error state if fetch failed
  if (error) {
    return <Renderer spec={errorSpec(error)} registry={registry} />;
  }

  // Dynamic spec from plugin ui_surfaces
  if (spec) {
    return <Renderer spec={spec} registry={registry} />;
  }

  // Fallback to React Router outlet
  return <Outlet />;
}
