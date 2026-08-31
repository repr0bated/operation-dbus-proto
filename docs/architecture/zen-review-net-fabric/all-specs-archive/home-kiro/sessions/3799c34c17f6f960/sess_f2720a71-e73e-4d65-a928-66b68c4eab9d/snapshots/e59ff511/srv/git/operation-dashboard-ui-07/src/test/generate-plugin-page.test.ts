import { describe, it, expect } from "vitest";
import { generatePluginPageSpec } from "@/json-render/spec-gen/generate-plugin-page";
import { validateGeneratedSpec } from "@/json-render/spec-gen/validate-spec";
import { CATALOG_COMPONENTS } from "@/json-render/catalog/catalog";
import type { UiSubidProjection } from "@/lib/subid-ui";

// Test fixtures
const minimalProjections: UiSubidProjection[] = [
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "status",
    subid: "obs.software.plugin.test.status@v1",
    category: "obs",
    role: "display-value",
    element_key: "software.plugin.test.status@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "connected",
    subid: "obs.software.plugin.test.connected@v1",
    category: "obs",
    role: "state-flag",
    element_key: "software.plugin.test.connected@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "count",
    subid: "mut.software.plugin.test.count@v1",
    category: "mut",
    role: "numeric-control",
    element_key: "software.plugin.test.count@v1",
  },
];

const projectionsWithCollections: UiSubidProjection[] = [
  ...minimalProjections,
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "items",
    subid: "obs.software.plugin.test.items@v1",
    category: "obs",
    role: "collection-view",
    element_key: "software.plugin.test.items@v1",
  },
];

const onlyOmittedProjections: UiSubidProjection[] = [
  {
    plugin_id: "test-plugin",
    kind: "schema",
    id: "schema",
    subid: "sch.software.plugin.test.schema@v1",
    category: "sch",
    role: "validation-carrier",
    element_key: "software.plugin.test.schema@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "surface",
    subid: "exp.software.plugin.test.surface@v1",
    category: "exp",
    role: "surface",
    element_key: "software.plugin.test.surface@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "hydration",
    subid: "src.software.plugin.test.hydration@v1",
    category: "src",
    role: "hydration-source",
    element_key: "software.plugin.test.hydration@v1",
  },
];

const mixedProjections: UiSubidProjection[] = [
  ...onlyOmittedProjections,
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "message",
    subid: "obs.software.plugin.test.message@v1",
    category: "obs",
    role: "display-value",
    element_key: "software.plugin.test.message@v1",
  },
];

const allRoleTypes: UiSubidProjection[] = [
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "displayVal",
    subid: "obs.software.plugin.test.display@v1",
    category: "obs",
    role: "display-value",
    element_key: "software.plugin.test.display@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "flag",
    subid: "obs.software.plugin.test.flag@v1",
    category: "obs",
    role: "state-flag",
    element_key: "software.plugin.test.flag@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "collection",
    subid: "obs.software.plugin.test.collection@v1",
    category: "obs",
    role: "collection-view",
    element_key: "software.plugin.test.collection@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "record",
    subid: "obs.software.plugin.test.record@v1",
    category: "obs",
    role: "record-view",
    element_key: "software.plugin.test.record@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "values",
    subid: "obs.software.plugin.test.values@v1",
    category: "obs",
    role: "value-list",
    element_key: "software.plugin.test.values@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "toggle",
    subid: "mut.software.plugin.test.toggle@v1",
    category: "mut",
    role: "binary-control",
    element_key: "software.plugin.test.toggle@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "text",
    subid: "mut.software.plugin.test.text@v1",
    category: "mut",
    role: "text-control",
    element_key: "software.plugin.test.text@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "number",
    subid: "mut.software.plugin.test.number@v1",
    category: "mut",
    role: "numeric-control",
    element_key: "software.plugin.test.number@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "choice",
    subid: "mut.software.plugin.test.choice@v1",
    category: "mut",
    role: "multi-choice",
    element_key: "software.plugin.test.choice@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "editList",
    subid: "mut.software.plugin.test.editList@v1",
    category: "mut",
    role: "editable-collection",
    element_key: "software.plugin.test.editList@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "editRecord",
    subid: "mut.software.plugin.test.editRecord@v1",
    category: "mut",
    role: "record-editor",
    element_key: "software.plugin.test.editRecord@v1",
  },
  {
    plugin_id: "test-plugin",
    kind: "field",
    id: "structured",
    subid: "mut.software.plugin.test.structured@v1",
    category: "mut",
    role: "structured-control",
    element_key: "software.plugin.test.structured@v1",
  },
];

describe("generatePluginPageSpec: element generation", () => {
  it("generates spec with expected elements from minimal projections", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      minimalProjections,
    );

    // Should have root element
    expect(spec.root).toBe("test-plugin--page");
    expect(spec.elements[spec.root]).toBeDefined();

    // Should have elements for each non-omitted projection
    const elements = spec.elements as Record<string, { type: string }>;
    expect(elements["test-plugin--status--display-value"]).toBeDefined();
    expect(elements["test-plugin--connected--state-flag"]).toBeDefined();
    expect(elements["test-plugin--count--numeric-control"]).toBeDefined();
  });

  it("sets root card title to displayName", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "My Custom Display Name",
      minimalProjections,
    );

    const rootElement = spec.elements[spec.root] as {
      props: { title: string };
    };
    expect(rootElement.props.title).toBe("My Custom Display Name");
  });

  it("root card children include all generated element IDs", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      minimalProjections,
    );

    const rootElement = spec.elements[spec.root] as { children: string[] };
    expect(rootElement.children).toContain("test-plugin--status--display-value");
    expect(rootElement.children).toContain("test-plugin--connected--state-flag");
    expect(rootElement.children).toContain("test-plugin--count--numeric-control");
    expect(rootElement.children).toHaveLength(3);
  });
});

describe("generatePluginPageSpec: catalog alignment", () => {
  it("all generated element types are in CATALOG_COMPONENTS", () => {
    const spec = generatePluginPageSpec("test-plugin", "Test Plugin", allRoleTypes);
    const catalogSet = new Set(CATALOG_COMPONENTS);

    for (const [key, element] of Object.entries(
      spec.elements as Record<string, { type: string }>,
    )) {
      expect(
        catalogSet.has(element.type),
        `Element "${key}" has type "${element.type}" not in CATALOG_COMPONENTS`,
      ).toBe(true);
    }
  });

  it("generates correct component types for each role", () => {
    const spec = generatePluginPageSpec("test-plugin", "Test Plugin", allRoleTypes);
    const elements = spec.elements as Record<string, { type: string }>;

    // Check role-to-component mappings
    expect(elements["test-plugin--displayVal--display-value"].type).toBe("stateValue");
    expect(elements["test-plugin--flag--state-flag"].type).toBe("statusDot");
    expect(elements["test-plugin--collection--collection-view"].type).toBe("streamObject");
    expect(elements["test-plugin--record--record-view"].type).toBe("card");
    expect(elements["test-plugin--values--value-list"].type).toBe("container");
    expect(elements["test-plugin--toggle--binary-control"].type).toBe("pill");
    expect(elements["test-plugin--text--text-control"].type).toBe("stateValue");
    expect(elements["test-plugin--number--numeric-control"].type).toBe("statCard");
    expect(elements["test-plugin--choice--multi-choice"].type).toBe("stateValue");
    expect(elements["test-plugin--editList--editable-collection"].type).toBe("streamObject");
    expect(elements["test-plugin--editRecord--record-editor"].type).toBe("card");
    expect(elements["test-plugin--structured--structured-control"].type).toBe("card");
  });
});

describe("generatePluginPageSpec: state paths", () => {
  it("all $state paths start with /plugins/", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      minimalProjections,
    );

    function findStatePaths(obj: unknown): string[] {
      const paths: string[] = [];

      function traverse(val: unknown): void {
        if (typeof val === "object" && val !== null) {
          if ("$state" in val && typeof (val as { $state: unknown }).$state === "string") {
            paths.push((val as { $state: string }).$state);
          }
          for (const v of Object.values(val)) {
            traverse(v);
          }
        }
      }

      traverse(obj);
      return paths;
    }

    const statePaths = findStatePaths(spec.elements);
    expect(statePaths.length).toBeGreaterThan(0);

    for (const path of statePaths) {
      expect(
        path.startsWith("/plugins/"),
        `State path "${path}" should start with /plugins/`,
      ).toBe(true);
    }
  });

  it("state paths include pluginId", () => {
    const spec = generatePluginPageSpec(
      "my-plugin",
      "My Plugin",
      minimalProjections.map((p) => ({ ...p, plugin_id: "my-plugin" })),
    );

    const elements = spec.elements as Record<
      string,
      { props: Record<string, unknown> }
    >;
    const statusElement = elements["my-plugin--status--display-value"];
    expect(statusElement.props.path).toBe("/plugins/my-plugin/status");
  });
});

describe("generatePluginPageSpec: validation", () => {
  it("validateGeneratedSpec returns valid for generated output", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      minimalProjections,
    );

    const result = validateGeneratedSpec(spec);
    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it("validateGeneratedSpec returns valid for all role types", () => {
    const spec = generatePluginPageSpec("test-plugin", "Test Plugin", allRoleTypes);

    const result = validateGeneratedSpec(spec);
    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it("validateGeneratedSpec returns valid for collections", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      projectionsWithCollections,
    );

    const result = validateGeneratedSpec(spec);
    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });
});

describe("generatePluginPageSpec: omitted roles", () => {
  it("projections with only omitted roles produce spec with just outer card", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      onlyOmittedProjections,
    );

    // Should have only the root element
    expect(spec.root).toBe("test-plugin--page");
    const elementCount = Object.keys(spec.elements).length;
    expect(elementCount).toBe(1);

    // Root card should have no children
    const rootElement = spec.elements[spec.root] as { children: string[] };
    expect(rootElement.children).toHaveLength(0);
  });

  it("mixed projections only include non-omitted roles as elements", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      mixedProjections,
    );

    const elements = spec.elements as Record<string, unknown>;

    // Should not have elements for omitted roles
    expect(elements["test-plugin--schema--validation-carrier"]).toBeUndefined();
    expect(elements["test-plugin--surface--surface"]).toBeUndefined();
    expect(elements["test-plugin--hydration--hydration-source"]).toBeUndefined();

    // Should have element for non-omitted role
    expect(elements["test-plugin--message--display-value"]).toBeDefined();
  });
});

describe("generatePluginPageSpec: element ID naming convention", () => {
  it("element IDs follow <pluginId>--<fieldId>--<role> convention", () => {
    const spec = generatePluginPageSpec(
      "my-plugin",
      "My Plugin",
      [
        {
          plugin_id: "my-plugin",
          kind: "field",
          id: "my-field",
          subid: "obs.software.plugin.test.my-field@v1",
          category: "obs",
          role: "display-value",
          element_key: "software.plugin.test.my-field@v1",
        },
      ],
    );

    const elementIds = Object.keys(spec.elements);
    expect(elementIds).toContain("my-plugin--page");
    expect(elementIds).toContain("my-plugin--my-field--display-value");
  });

  it("root element ID is <pluginId>--page", () => {
    const spec = generatePluginPageSpec(
      "custom-plugin",
      "Custom Plugin",
      minimalProjections,
    );

    expect(spec.root).toBe("custom-plugin--page");
  });

  it("all non-root element IDs are deterministic based on projection data", () => {
    const projections: UiSubidProjection[] = [
      {
        plugin_id: "plugin-a",
        kind: "field",
        id: "field-x",
        subid: "obs.software.plugin.test.x@v1",
        category: "obs",
        role: "display-value",
        element_key: "software.plugin.test.x@v1",
      },
      {
        plugin_id: "plugin-a",
        kind: "field",
        id: "field-y",
        subid: "obs.software.plugin.test.y@v1",
        category: "obs",
        role: "state-flag",
        element_key: "software.plugin.test.y@v1",
      },
    ];

    // Generate twice, should produce identical IDs
    const spec1 = generatePluginPageSpec("plugin-a", "Plugin A", projections);
    const spec2 = generatePluginPageSpec("plugin-a", "Plugin A", projections);

    expect(Object.keys(spec1.elements).sort()).toEqual(
      Object.keys(spec2.elements).sort(),
    );
    expect(spec1.root).toBe(spec2.root);
  });
});

describe("generatePluginPageSpec: repeat blocks", () => {
  it("collection roles have repeat blocks", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      projectionsWithCollections,
    );

    const elements = spec.elements as Record<
      string,
      { repeat?: { over: { $state: string }; as: string } }
    >;

    const collectionElement = elements["test-plugin--items--collection-view"];
    expect(collectionElement.repeat).toBeDefined();
    expect(collectionElement.repeat?.over.$state).toBe("/plugins/test-plugin/items");
    expect(collectionElement.repeat?.as).toBe("item");
  });

  it("non-collection roles do not have repeat blocks", () => {
    const spec = generatePluginPageSpec(
      "test-plugin",
      "Test Plugin",
      minimalProjections,
    );

    const elements = spec.elements as Record<
      string,
      { repeat?: unknown }
    >;

    expect(elements["test-plugin--status--display-value"].repeat).toBeUndefined();
    expect(elements["test-plugin--connected--state-flag"].repeat).toBeUndefined();
    expect(elements["test-plugin--count--numeric-control"].repeat).toBeUndefined();
  });
});

describe("validateGeneratedSpec: error detection", () => {
  it("detects missing root element", () => {
    const invalidSpec = {
      root: "missing-root",
      elements: {
        "some-element": { type: "card", props: {} },
      },
    };

    const result = validateGeneratedSpec(invalidSpec as unknown as import("@json-render/core").Spec);
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("missing-root"))).toBe(true);
  });

  it("detects unknown component type", () => {
    const invalidSpec = {
      root: "root",
      elements: {
        root: { type: "unknownComponent", props: {} },
      },
    };

    const result = validateGeneratedSpec(invalidSpec as unknown as import("@json-render/core").Spec);
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("unknownComponent"))).toBe(true);
  });

  it("detects invalid $state paths", () => {
    const invalidSpec = {
      root: "root",
      elements: {
        root: {
          type: "card",
          props: { value: { $state: "no-leading-slash" } },
        },
      },
    };

    const result = validateGeneratedSpec(invalidSpec as unknown as import("@json-render/core").Spec);
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("must start with /"))).toBe(true);
  });

  it("detects missing children references", () => {
    const invalidSpec = {
      root: "root",
      elements: {
        root: {
          type: "card",
          props: {},
          children: ["nonexistent-child"],
        },
      },
    };

    const result = validateGeneratedSpec(invalidSpec as unknown as import("@json-render/core").Spec);
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("nonexistent-child"))).toBe(true);
  });
});
