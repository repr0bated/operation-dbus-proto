import { describe, it, expect } from "vitest";
import { UI_ROLES, type UiRole } from "@/lib/subid-ui";
import {
  ROLE_MAP,
  ALL_UI_ROLES,
  componentForRole,
  isOmittedRole,
  isReadOnlyRole,
  needsRepeat,
  staticPropsForRole,
} from "@/json-render/catalog/role-map";
import { CATALOG_COMPONENTS } from "@/json-render/catalog/catalog";

describe("role-map: completeness", () => {
  it("ROLE_MAP has an entry for every UiRole", () => {
    for (const role of UI_ROLES) {
      expect(ROLE_MAP[role], `missing ROLE_MAP entry for '${role}'`).toBeDefined();
    }
  });

  it("ALL_UI_ROLES matches UI_ROLES", () => {
    expect(ALL_UI_ROLES.slice().sort()).toEqual([...UI_ROLES].sort());
  });

  it("every UiRole is covered exactly once", () => {
    const mapped = new Set(Object.keys(ROLE_MAP));
    const expected = new Set(UI_ROLES);
    expect(mapped).toEqual(expected);
  });
});

describe("role-map: catalog alignment", () => {
  it("non-omitted roles map to valid catalog component names", () => {
    const catalogSet = new Set(CATALOG_COMPONENTS);

    for (const role of UI_ROLES) {
      const mapping = ROLE_MAP[role];
      if (mapping.omit) continue;

      const component = mapping.component;
      expect(
        component,
        `role '${role}' has omit=false but no component`,
      ).toBeDefined();

      expect(
        catalogSet.has(component!),
        `role '${role}' maps to '${component}' which is not in CATALOG_COMPONENTS`,
      ).toBe(true);
    }
  });

  it("omitted roles have no component", () => {
    const omittedRoles: UiRole[] = [
      "surface",
      "validation-carrier",
      "hydration-source",
      "trigger-binding",
      "repeat-binding",
    ];

    for (const role of omittedRoles) {
      expect(ROLE_MAP[role].omit, `expected '${role}' to be omitted`).toBe(true);
      expect(
        ROLE_MAP[role].component,
        `omitted role '${role}' should not have a component`,
      ).toBeUndefined();
    }
  });
});

describe("role-map: helper functions", () => {
  it("componentForRole returns component or undefined for omitted", () => {
    expect(componentForRole("display-value")).toBe("stateValue");
    expect(componentForRole("state-flag")).toBe("statusDot");
    expect(componentForRole("collection-view")).toBe("streamObject");
    expect(componentForRole("surface")).toBeUndefined();
    expect(componentForRole("validation-carrier")).toBeUndefined();
  });

  it("isOmittedRole identifies omitted roles correctly", () => {
    expect(isOmittedRole("surface")).toBe(true);
    expect(isOmittedRole("validation-carrier")).toBe(true);
    expect(isOmittedRole("hydration-source")).toBe(true);
    expect(isOmittedRole("trigger-binding")).toBe(true);
    expect(isOmittedRole("repeat-binding")).toBe(true);

    expect(isOmittedRole("display-value")).toBe(false);
    expect(isOmittedRole("state-flag")).toBe(false);
    expect(isOmittedRole("numeric-control")).toBe(false);
  });

  it("isReadOnlyRole identifies read-only controls", () => {
    // Read-only controls (mut category, no edit yet)
    expect(isReadOnlyRole("binary-control")).toBe(true);
    expect(isReadOnlyRole("text-control")).toBe(true);
    expect(isReadOnlyRole("numeric-control")).toBe(true);
    expect(isReadOnlyRole("multi-choice")).toBe(true);
    expect(isReadOnlyRole("editable-collection")).toBe(true);
    expect(isReadOnlyRole("record-editor")).toBe(true);
    expect(isReadOnlyRole("structured-control")).toBe(true);

    // Obs roles are not "read-only" in the control sense — they are always read-only
    expect(isReadOnlyRole("display-value")).toBe(false);
    expect(isReadOnlyRole("state-flag")).toBe(false);
    expect(isReadOnlyRole("collection-view")).toBe(false);
  });

  it("needsRepeat identifies collection roles", () => {
    expect(needsRepeat("collection-view")).toBe(true);
    expect(needsRepeat("value-list")).toBe(true);
    expect(needsRepeat("editable-collection")).toBe(true);

    expect(needsRepeat("display-value")).toBe(false);
    expect(needsRepeat("record-view")).toBe(false);
    expect(needsRepeat("surface")).toBe(false);
  });

  it("staticPropsForRole returns static props or empty object", () => {
    expect(staticPropsForRole("numeric-control")).toEqual({
      sub: null,
      variant: null,
      tone: null,
    });
    expect(staticPropsForRole("multi-choice")).toEqual({ format: "raw" });
    expect(staticPropsForRole("binary-control")).toEqual({ tone: null });

    // Roles without static props return empty object
    expect(staticPropsForRole("display-value")).toEqual({});
    expect(staticPropsForRole("surface")).toEqual({});
  });
});

describe("role-map: design constraints", () => {
  it("surface is omitted (becomes route, not element)", () => {
    expect(ROLE_MAP["surface"].omit).toBe(true);
  });

  it("all obs roles (display, state-flag, collection, record, value-list) are not read-only flagged", () => {
    // obs roles are inherently read-only by nature; the readOnly flag is for mut controls
    const obsRoles: UiRole[] = [
      "display-value",
      "state-flag",
      "collection-view",
      "record-view",
      "value-list",
    ];
    for (const role of obsRoles) {
      expect(
        ROLE_MAP[role].readOnly,
        `obs role '${role}' should not have readOnly flag`,
      ).toBeFalsy();
    }
  });

  it("all mut roles are flagged read-only (controls are future work)", () => {
    const mutRoles: UiRole[] = [
      "binary-control",
      "text-control",
      "numeric-control",
      "multi-choice",
      "editable-collection",
      "record-editor",
      "structured-control",
    ];
    for (const role of mutRoles) {
      expect(
        ROLE_MAP[role].readOnly,
        `mut role '${role}' should be read-only in this phase`,
      ).toBe(true);
    }
  });
});
