/**
 * TypeScript mirror of `op-state-store/src/subid_ui.rs`.
 *
 * Types only — no logic. The Rust side is authoritative; this file exists so
 * the spec generator can work with the same type vocabulary.
 */

/**
 * The seven OSCAL subid categories from docs/subid-taxonomy.md.
 */
export const SUBID_CATEGORIES = ["src", "prj", "sch", "mut", "obs", "evt", "exp"] as const;
export type SubidCategory = (typeof SUBID_CATEGORIES)[number];

/**
 * Presentation roles — data-semantic only (no json-render component names).
 *
 * These are the kebab-case strings from `UiRole::as_str()` in Rust.
 */
export const UI_ROLES = [
  "surface",
  "display-value",
  "state-flag",
  "collection-view",
  "record-view",
  "value-list",
  "binary-control",
  "text-control",
  "numeric-control",
  "multi-choice",
  "editable-collection",
  "record-editor",
  "structured-control",
  "validation-carrier",
  "hydration-source",
  "trigger-binding",
  "repeat-binding",
] as const;

export type UiRole = (typeof UI_ROLES)[number];

/**
 * Normalized field shape used to refine obs/mut roles.
 * Derived from schema field types when available.
 */
export type UiFieldShape =
  | "boolean"
  | "string"
  | "integer"
  | "float"
  | "list-scalar"
  | "list-record"
  | "record"
  | "any";

/**
 * One populated catalog row for dump / API.
 * Mirrors `UiSubidProjection` in Rust.
 *
 * Returned by `GET /api/ui-model/plugin-schema/:plugin` in the `ui_projection` field.
 */
export interface UiSubidProjection {
  /** Plugin that owns this projection. */
  plugin_id: string;
  /** "field" | "method" | "schema" */
  kind: string;
  /** Field or method name. */
  id: string;
  /** Full subid string, e.g. "obs.software.plugin.xray.status@v1" */
  subid: string;
  /** Category extracted from subid (obs, mut, exp, ...) */
  category: SubidCategory | string;
  /** Presentation role (display-value, state-flag, ...) */
  role: UiRole;
  /** Unique join key = subid segments after category. */
  element_key: string;
}

/**
 * A plugin-owned UI surface route.
 * Mirrors `UiSurfaceRoute` in op-gallery-gen context.rs.
 */
export interface UiSurfaceRoute {
  /** Route path, e.g. "/antigravity/chat" */
  path: string;
  /** Human-readable name for nav */
  name?: string;
  /** Schema scope (which fields this surface shows) */
  schema?: string;
  /** Original raw object from the plugin */
  raw?: unknown;
}

/**
 * A plugin's ui_surfaces projection.
 * Mirrors `UiSurfaceProjection` in op-gallery-gen context.rs.
 */
export interface UiSurfaceProjection {
  /** Subids that declared ui_surfaces (for traceability) */
  subids: string[];
  /** Authoritative routes */
  routes: UiSurfaceRoute[];
  /** Where the routes came from ("default" | "example" | null) */
  value_source?: string;
}

/**
 * Check if a ui_surfaces projection is authoritative (has subids and routes).
 */
export function isAuthoritative(projection: UiSurfaceProjection | null | undefined): boolean {
  if (!projection) return false;
  return projection.subids.length > 0 && projection.routes.length > 0;
}

/**
 * Extract the category (first segment) from a subid.
 * Returns null if the category is not one of the seven known categories.
 */
export function subidCategory(subid: string): SubidCategory | null {
  const base = subid.split("@")[0] ?? subid;
  const cat = base.split(".")[0];
  if (cat && (SUBID_CATEGORIES as readonly string[]).includes(cat)) {
    return cat as SubidCategory;
  }
  return null;
}

/**
 * Derive a UiRole from a subid + optional field shape.
 * Mirrors `ui_role_from_subid` in Rust.
 */
export function uiRoleFromSubid(subid: string, shape?: UiFieldShape): UiRole | null {
  const cat = subidCategory(subid);
  if (!cat) return null;

  switch (cat) {
    case "exp":
      return "surface";
    case "sch":
      return "validation-carrier";
    case "src":
      return "hydration-source";
    case "evt":
      return "trigger-binding";
    case "prj":
      return "repeat-binding";
    case "obs":
      switch (shape) {
        case "boolean":
          return "state-flag";
        case "list-record":
          return "collection-view";
        case "record":
          return "record-view";
        case "list-scalar":
          return "value-list";
        default:
          return "display-value";
      }
    case "mut":
      switch (shape) {
        case "boolean":
          return "binary-control";
        case "string":
          return "text-control";
        case "integer":
        case "float":
          return "numeric-control";
        case "list-scalar":
          return "multi-choice";
        case "list-record":
          return "editable-collection";
        case "record":
          return "record-editor";
        default:
          return "structured-control";
      }
    default:
      return null;
  }
}

/**
 * Strip category from subid; remainder is the unique element key.
 * Mirrors `element_key_from_subid` in Rust.
 */
export function elementKeyFromSubid(subid: string): string {
  const [base, ver] = subid.split("@");
  const parts = (base ?? "").split(".");
  parts.shift(); // remove category
  const key = parts.join(".");
  if (ver && key) return `${key}@${ver}`;
  if (ver) return `@${ver}`;
  return key;
}
