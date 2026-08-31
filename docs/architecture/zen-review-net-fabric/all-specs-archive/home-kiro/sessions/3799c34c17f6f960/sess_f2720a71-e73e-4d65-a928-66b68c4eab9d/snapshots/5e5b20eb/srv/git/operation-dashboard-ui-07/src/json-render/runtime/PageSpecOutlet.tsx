/**
 * Route-aware page outlet.
 *
 * Registered PAGE_SPECS win. `/plugins/:id` (not `/plugins` itself) renders
 * the sealed-schema ui_projection. Everything else falls through to <Outlet />.
 */
import { useMemo } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { Renderer } from "@json-render/react";
import { useJsonRender } from "./JsonRenderProvider";
import { pageSpecFor, PAGE_SPECS } from "@/json-render/pages";
import { ProjectedPluginPage } from "./ProjectedPluginPage";

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

export function PageSpecOutlet() {
  const { pathname } = useLocation();
  const { registry } = useJsonRender();
  const spec = useMemo(() => pageSpecFor(pathname), [pathname]);
  const pluginId = spec ? null : pluginIdFromPath(pathname);

  if (spec) return <Renderer spec={spec} registry={registry} />;
  if (pluginId) return <ProjectedPluginPage pluginId={pluginId} />;
  return <Outlet />;
}
