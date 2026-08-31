/**
 * Public entry for the JSON Render base.
 *
 * ── Adding a screen ──────────────────────────────────────────────────────
 *   1. `navigation/manifest.ts` — one NAV_MANIFEST entry (label, route, icon,
 *      section, order). The sidebar picks it up automatically.
 *   2. `pages/index.ts`        — map that route to a page spec.
 * Skip step 2 to keep a legacy React page for the route.
 * ─────────────────────────────────────────────────────────────────────────
 */

// Catalog + registry
export {
  appCatalog,
  registry,
  baseRegistry,
  handlers,
  executeAction,
  CATALOG_COMPONENTS,
  CATALOG_ACTIONS,
  type AppCatalog,
} from "./catalog";

// Runtime
export { JsonRenderProvider, useJsonRender } from "./runtime/JsonRenderProvider";
export { PageSpecOutlet } from "./runtime/PageSpecOutlet";
export { SpecPage } from "./runtime/SpecPage";
export { SlotProvider, useSlot, CONTENT_SLOT } from "./runtime/slots";
export { RouterBridge, navigateTo } from "./runtime/navigation-bridge";
export { ProjectedPluginPage } from "./runtime/ProjectedPluginPage";
export {
  projectionToSpec,
  catalogTypeForRole,
  ROLE_TO_CATALOG,
  LIVE_PROJECTION_ROLES,
} from "./projection";

// Shell
export { ShellRenderer } from "./shell/ShellRenderer";
export {
  buildShellSpec,
  shellSpec,
  shellInitialState,
  SHELL_STATE,
  DEFAULT_CHROME,
  type ShellChrome,
} from "./shell/shellSpec";

// Pages
export { PAGE_SPECS, pageSpecFor, overviewSpec } from "./pages";

// Navigation
export {
  NAV_MANIFEST,
  SECTION_ORDER,
  buildNavGroups,
  sectionSlug,
  activeSectionSlug,
  type NavItem,
  type NavGroup,
  type NavSection,
} from "./navigation/manifest";
export {
  ALLOW_ALL,
  hasCapability,
  hasAll,
  type Capability,
  type CapabilitySet,
} from "./navigation/capabilities";
export { resolveIcon, iconRegistry } from "./navigation/icons";

// Spec builders for runtime JSON
export { dataToSpec, streamToSpec } from "./spec-builders";
