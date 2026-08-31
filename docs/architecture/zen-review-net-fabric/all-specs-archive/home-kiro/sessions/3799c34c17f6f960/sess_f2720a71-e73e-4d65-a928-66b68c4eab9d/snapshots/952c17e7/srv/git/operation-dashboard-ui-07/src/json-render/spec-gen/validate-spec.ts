/**
 * Spec Validator
 *
 * Client-side fast validator for json-render specs.
 * Validates structure and references before rendering.
 */
import type { Spec } from "@json-render/core";
import { CATALOG_COMPONENTS } from "@/json-render/catalog/catalog";

/**
 * Result of spec validation.
 */
export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

/**
 * Check if a value is a $state binding.
 */
function isStateBinding(value: unknown): value is { $state: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "$state" in value &&
    typeof (value as { $state: unknown }).$state === "string"
  );
}

/**
 * Validate that a $state path starts with /.
 */
function validateStatePath(path: string, context: string): string | null {
  if (!path.startsWith("/")) {
    return `${context}: $state path "${path}" must start with /`;
  }
  return null;
}

/**
 * Recursively find and validate all $state bindings in props.
 */
function validateStateBindings(
  props: Record<string, unknown>,
  elementId: string,
): string[] {
  const errors: string[] = [];

  function traverse(obj: unknown, path: string): void {
    if (isStateBinding(obj)) {
      const error = validateStatePath(obj.$state, `${elementId}.${path}`);
      if (error) {
        errors.push(error);
      }
    } else if (typeof obj === "object" && obj !== null) {
      if (Array.isArray(obj)) {
        obj.forEach((item, index) => traverse(item, `${path}[${index}]`));
      } else {
        for (const [key, value] of Object.entries(obj)) {
          traverse(value, path ? `${path}.${key}` : key);
        }
      }
    }
  }

  traverse(props, "props");
  return errors;
}

/**
 * Validate a generated spec for correctness.
 *
 * Checks:
 * - root field exists and references a valid element ID
 * - Every element has a `type` field matching a CATALOG_COMPONENTS entry
 * - Every `$state` path starts with `/`
 * - children arrays reference existing element IDs
 *
 * @param spec - The Spec to validate
 * @returns ValidationResult with valid flag and any errors
 */
export function validateGeneratedSpec(spec: Spec): ValidationResult {
  const errors: string[] = [];
  const catalogSet = new Set(CATALOG_COMPONENTS);

  // Type assertion for accessing spec properties
  const elements = spec.elements as Record<
    string,
    {
      type?: string;
      props?: Record<string, unknown>;
      children?: string[];
    }
  >;
  const root = spec.root as string;

  // Check root field exists
  if (!root) {
    errors.push("Spec is missing required 'root' field");
    return { valid: false, errors };
  }

  // Check root references a valid element
  if (!elements || typeof elements !== "object") {
    errors.push("Spec is missing required 'elements' field");
    return { valid: false, errors };
  }

  if (!elements[root]) {
    errors.push(`Root element "${root}" does not exist in elements`);
  }

  // Get all element IDs for children validation
  const elementIds = new Set(Object.keys(elements));

  // Validate each element
  for (const [elementId, element] of Object.entries(elements)) {
    // Check type field exists
    if (!element.type) {
      errors.push(`Element "${elementId}" is missing required 'type' field`);
      continue;
    }

    // Check type is in catalog
    if (!catalogSet.has(element.type)) {
      errors.push(
        `Element "${elementId}" has unknown type "${element.type}" not in CATALOG_COMPONENTS`,
      );
    }

    // Validate $state paths in props
    if (element.props && typeof element.props === "object") {
      const stateErrors = validateStateBindings(element.props, elementId);
      errors.push(...stateErrors);
    }

    // Validate children references
    if (element.children) {
      if (!Array.isArray(element.children)) {
        errors.push(`Element "${elementId}" has non-array children field`);
      } else {
        for (const childId of element.children) {
          if (!elementIds.has(childId)) {
            errors.push(
              `Element "${elementId}" references non-existent child "${childId}"`,
            );
          }
        }
      }
    }
  }

  return {
    valid: errors.length === 0,
    errors,
  };
}
