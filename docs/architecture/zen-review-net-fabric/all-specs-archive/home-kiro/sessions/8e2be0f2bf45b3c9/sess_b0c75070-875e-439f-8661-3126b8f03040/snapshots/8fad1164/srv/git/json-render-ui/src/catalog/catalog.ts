import { defineCatalog } from "@json-render/core";
import { schema } from "@json-render/react/schema";
import { z } from "zod";

export const appCatalog = defineCatalog(schema, {
  components: {
    // Shell
    shell: {
      props: z.object({
        navCollapsed: z.boolean().nullable(),
      }),
      slots: ["nav", "header", "content"],
      description: "Root shell layout with sidebar navigation, header, and content area.",
    },
    navItem: {
      props: z.object({
        label: z.string(),
        route: z.string(),
        icon: z.string().nullable(),
        active: z.boolean().nullable(),
      }),
      description: "Sidebar navigation item linking to a route.",
    },
    pageHeader: {
      props: z.object({
        title: z.string(),
        subtitle: z.string().nullable(),
      }),
      description: "Page title and optional subtitle at the top of the content area.",
    },

    // Layout
    container: {
      props: z.object({
        className: z.string().nullable(),
      }),
      slots: ["default"],
      description: "Generic flex container for layout composition.",
    },
    grid: {
      props: z.object({
        cols: z.number(),
        gap: z.number().nullable(),
        className: z.string().nullable(),
      }),
      slots: ["default"],
      description: "CSS grid layout with configurable columns.",
    },
    card: {
      props: z.object({
        title: z.string().nullable(),
        className: z.string().nullable(),
      }),
      slots: ["default"],
      description: "Bordered card with optional title.",
    },

    // Data display
    kv: {
      props: z.object({
        label: z.string(),
        value: z.unknown(),
        kind: z.string().nullable(),
      }),
      description: "Key-value pair display. Kind hints the value type (string, number, boolean, etc).",
    },
    text: {
      props: z.object({
        text: z.string(),
        className: z.string().nullable(),
      }),
      description: "Plain text content.",
    },
    badge: {
      props: z.object({
        label: z.string(),
        tone: z.enum(["default", "ok", "warn", "danger", "info"]).nullable(),
      }),
      description: "Colored status badge.",
    },
    stat: {
      props: z.object({
        label: z.string(),
        value: z.string(),
        unit: z.string().nullable(),
        trend: z.enum(["up", "down", "flat"]).nullable(),
      }),
      description: "Single metric with label, value, optional unit and trend arrow.",
    },

    // Live data
    pluginState: {
      props: z.object({
        pluginId: z.string(),
      }),
      slots: ["default"],
      description: "Subscribes to a plugin's live state and renders children with that data.",
    },
    stateValue: {
      props: z.object({
        path: z.string(),
        label: z.string().nullable(),
        format: z.enum(["raw", "json", "bytes", "duration"]).nullable(),
      }),
      description: "Displays a single value from the live state store by dot-path.",
    },
  },
  actions: {
    navigate: {
      params: z.object({ route: z.string() }),
      description: "Navigate the app to a route path.",
    },
    callMethod: {
      params: z.object({
        subid: z.string(),
        input: z.record(z.string(), z.unknown()),
      }),
      description: "Invoke a plugin method by its OSCAL subid.",
    },
  },
});
