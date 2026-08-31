/**
 * Plugin Page Spec Generator
 *
 * Generates a json-render Spec from plugin projections using the role-map
 * to determine component types and props.
 */
import type { Spec } from "@json-render/core";
import type { UiSubidProjection, UiRole } from "@/lib/subid-ui";
import {
  componentForRole,
  isOmittedRole,
  needsRepeat,
  staticPropsForRole,
} from "@/json-render/catalog/role-map";

/**
 * Element definition for building specs.
 */
interface Element {
  type: string;
  props: Record<string, unknown>;
  children?: string[];
  repeat?: {
    over: { $state: string };
    as: string;
  };
}

/**
 * Generate a deterministic element ID following the naming convention.
 * Format: <pluginId>--<fieldId>--<role>
 */
function elementId(pluginId: string, fieldId: string, role: string): string {
  return `${pluginId}--${fieldId}--${role}`;
}

/**
 * Generate a state path for a plugin field.
 * Format: /plugins/<pluginId>/<fieldName>
 */
function statePath(pluginId: string, fieldName: string): { $state: string } {
  return { $state: `/plugins/${pluginId}/${fieldName}` };
}

/**
 * Build props for an element based on its role.
 */
function buildPropsForRole(
  pluginId: string,
  fieldId: string,
  role: UiRole,
): Record<string, unknown> {
  const staticProps = staticPropsForRole(role);
  const path = statePath(pluginId, fieldId);

  switch (role) {
    case "display-value":
    case "text-control":
    case "multi-choice":
      // stateValue: path + label
      return {
        ...staticProps,
        path: path.$state,
        label: fieldId,
      };

    case "state-flag":
      // statusDot: status bound via $state
      return {
        ...staticProps,
        status: path,
      };

    case "numeric-control":
      // statCard: label + value bound via $state
      return {
        ...staticProps,
        label: fieldId,
        value: path,
      };

    case "collection-view":
    case "editable-collection":
      // streamObject: pluginId + member
      return {
        ...staticProps,
        pluginId,
        member: fieldId,
      };

    case "record-view":
    case "record-editor":
    case "structured-control":
      // card: title + optional content
      return {
        ...staticProps,
        title: fieldId,
        subtitle: null,
        tone: null,
        className: null,
      };

    case "value-list":
      // container for list of scalars
      return {
        ...staticProps,
        className: null,
      };

    case "binary-control":
      // pill: text bound via $state
      return {
        ...staticProps,
        text: path,
        variant: null,
      };

    default:
      return { ...staticProps };
  }
}

/**
 * Build an element from a projection.
 */
function buildElement(
  pluginId: string,
  projection: UiSubidProjection,
): Element | null {
  const role = projection.role as UiRole;

  if (isOmittedRole(role)) {
    return null;
  }

  const component = componentForRole(role);
  if (!component) {
    return null;
  }

  const props = buildPropsForRole(pluginId, projection.id, role);

  const element: Element = {
    type: component,
    props,
  };

  // Add repeat block for collection roles
  if (needsRepeat(role)) {
    element.repeat = {
      over: statePath(pluginId, projection.id),
      as: "item",
    };
  }

  return element;
}

/**
 * Generate a json-render Spec from plugin projections.
 *
 * @param pluginId - The plugin identifier
 * @param displayName - Human-readable display name for the page title
 * @param projections - Array of UiSubidProjection from the plugin
 * @returns A valid Spec for json-render
 */
export function generatePluginPageSpec(
  pluginId: string,
  displayName: string,
  projections: UiSubidProjection[],
): Spec {
  const elements: Record<string, Element> = {};
  const childIds: string[] = [];

  // Process each projection
  for (const projection of projections) {
    const role = projection.role as UiRole;

    // Skip omitted roles
    if (isOmittedRole(role)) {
      continue;
    }

    const element = buildElement(pluginId, projection);
    if (!element) {
      continue;
    }

    const id = elementId(pluginId, projection.id, role);
    elements[id] = element;
    childIds.push(id);
  }

  // Create root card element
  const rootId = `${pluginId}--page`;
  elements[rootId] = {
    type: "card",
    props: {
      title: displayName,
      subtitle: null,
      tone: null,
      className: null,
    },
    children: childIds,
  };

  return {
    root: rootId,
    elements,
  } as Spec;
}
