/**
 * Role-to-component mapping — the missing step.
 *
 * This is the sole point of contact between `UiRole` (semantics from
 * op-state-store/subid_ui.rs) and catalog component names (syntax from
 * catalog.ts). No other file may hardcode catalog component names based on
 * subid category.
 *
 * The mapping determines what component a generated spec may use for a given
 * projected field.
 */
import type { UiRole } from "@/lib/subid-ui";

/**
 * How a UiRole maps to a catalog component.
 */
export interface RoleMapping {
  /**
   * Primary catalog component type for this role.
   * Undefined when `omit` is true.
   */
  component?: string;

  /**
   * Props to merge in (static, not state-bound).
   * These are defaults that can be overridden by the spec generator.
   */
  staticProps?: Record<string, unknown>;

  /**
   * When true, the mapped element needs a `repeat` block for collections.
   */
  useRepeat?: boolean;

  /**
   * When true, the role produces no rendered element and is silently omitted.
   * Used for meta-roles like validation-carrier, hydration-source, etc.
   */
  omit?: boolean;

  /**
   * When true, the component is read-only in this phase.
   * Controls will be editable in a future phase.
   */
  readOnly?: boolean;
}

/**
 * The role-to-component map.
 *
 * Every UiRole must have an entry here. The generator uses this to decide
 * which catalog component to emit for a given field's role.
 *
 * Design notes:
 * - `surface` becomes a route, not a rendered element — it drives nav
 * - `display-value` → `stateValue` — shows a single bound value
 * - `state-flag` → `statusDot` — boolean as health indicator
 * - `collection-view` → `streamObject` with repeat — shows list of records
 * - `record-view` → `card` — shows a single record as grouped kv pairs
 * - `value-list` → `container` with repeat → `kv` — shows list of scalars
 * - `binary-control` → `pill` (read-only) — boolean toggle, read-only for now
 * - `text-control` → `stateValue` (read-only) — string input, read-only for now
 * - `numeric-control` → `statCard` — number input, read-only for now
 * - `multi-choice` → `stateValue` with raw format — enum/list selection
 * - `editable-collection` → `streamObject` with repeat — editable list of records
 * - `record-editor` → `card` (read-only) — editable record form
 * - `structured-control` → `card` (read-only) — complex nested structure
 * - validation-carrier, hydration-source, trigger-binding, repeat-binding → omitted
 */
export const ROLE_MAP: Record<UiRole, RoleMapping> = {
  // `exp.*` — consumer-facing surface → becomes a route, not an element
  "surface": {
    omit: true,
  },

  // `obs` scalar — read-only value
  "display-value": {
    component: "stateValue",
  },

  // `obs` boolean — on/off state
  "state-flag": {
    component: "statusDot",
    staticProps: { status: "ok" }, // overridden by $state binding at render
  },

  // `obs` list of records
  "collection-view": {
    component: "streamObject",
    useRepeat: true,
  },

  // `obs` record
  "record-view": {
    component: "card",
  },

  // `obs` list of scalars
  "value-list": {
    component: "container",
    useRepeat: true,
  },

  // `mut` boolean — read-only for now
  "binary-control": {
    component: "pill",
    staticProps: { tone: null },
    readOnly: true,
  },

  // `mut` string — read-only for now
  "text-control": {
    component: "stateValue",
    readOnly: true,
  },

  // `mut` integer / number
  "numeric-control": {
    component: "statCard",
    staticProps: { sub: null, variant: null, tone: null },
    readOnly: true,
  },

  // `mut` list of scalars
  "multi-choice": {
    component: "stateValue",
    staticProps: { format: "raw" },
    readOnly: true,
  },

  // `mut` list of records
  "editable-collection": {
    component: "streamObject",
    useRepeat: true,
    readOnly: true,
  },

  // `mut` record
  "record-editor": {
    component: "card",
    readOnly: true,
  },

  // `mut` otherwise / unstructured
  "structured-control": {
    component: "card",
    readOnly: true,
  },

  // `sch.*` — validation attaches to controls, not rendered directly
  "validation-carrier": {
    omit: true,
  },

  // `src.*` — hydration / ingress, not rendered directly
  "hydration-source": {
    omit: true,
  },

  // `evt.*` — trigger / audit binding, not rendered directly
  "trigger-binding": {
    omit: true,
  },

  // `prj.*` — projection / repeat binding, not rendered directly
  "repeat-binding": {
    omit: true,
  },
};

/**
 * Get the catalog component name for a UiRole.
 * Returns undefined if the role should be omitted.
 */
export function componentForRole(role: UiRole): string | undefined {
  const mapping = ROLE_MAP[role];
  if (mapping.omit) return undefined;
  return mapping.component;
}

/**
 * Check if a role should be omitted from generated specs.
 */
export function isOmittedRole(role: UiRole): boolean {
  return ROLE_MAP[role]?.omit === true;
}

/**
 * Check if a role is read-only in this phase.
 */
export function isReadOnlyRole(role: UiRole): boolean {
  return ROLE_MAP[role]?.readOnly === true;
}

/**
 * Check if a role needs a repeat block for collections.
 */
export function needsRepeat(role: UiRole): boolean {
  return ROLE_MAP[role]?.useRepeat === true;
}

/**
 * Get static props for a role, if any.
 */
export function staticPropsForRole(role: UiRole): Record<string, unknown> {
  return ROLE_MAP[role]?.staticProps ?? {};
}

/**
 * All UiRole values in a predictable order for iteration.
 */
export const ALL_UI_ROLES: UiRole[] = [
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
];
