/**
 * The catalog — the single contract for everything a json-render spec may
 * contain in this app. Nothing renders that is not declared here.
 *
 * Stream objects: every sealed plugin that arrives on StateSync.Subscribe is
 * a catalog UI object (camelCase name). Specs compose them; implementations
 * emit press/select; live values are read from /plugins/<id>.
 */
import { defineCatalog } from "@json-render/core";
import { schema } from "@json-render/react/schema";
import { z } from "zod";
import { STREAM_PLUGIN_IDS, toCatalogName } from "./stream-plugins";

const streamPluginProps = z.object({
  member: z.string().nullable(),
  className: z.string().nullable(),
});

function streamPluginDecl(pluginId: string) {
  return {
    props: streamPluginProps,
    events: ["press", "select"],
    description: `Live StateSync object for sealed plugin '${pluginId}'. Reads /plugins/${pluginId} from the event stream. Emits press/select.`,
  };
}

const streamPluginComponents = Object.fromEntries(
  STREAM_PLUGIN_IDS.map((id) => [toCatalogName(id), streamPluginDecl(id)]),
);

export const appCatalog = defineCatalog(schema, {
  components: {
    // Shell
    shell: {
      props: z.object({
        navCollapsed: z.boolean().nullable(),
        navWidth: z.string().nullable(),
        collapsedNavWidth: z.string().nullable(),
        topbarHeight: z.string().nullable(),
      }),
      slots: ["topbar", "nav", "content"],
      description: "Root chrome: topbar row, sidebar, content. Same grid as the old appShell.",
    },
    topbar: {
      props: z.object({}),
      slots: ["default"],
      description: "Fixed top header bar.",
    },
    topbarGroup: {
      props: z.object({
        align: z.enum(["start", "end"]).nullable(),
      }),
      slots: ["default"],
      description: "Horizontal cluster inside the topbar.",
    },
    navToggle: {
      props: z.object({
        collapsed: z.boolean().nullable(),
        expandTitle: z.string(),
        collapseTitle: z.string(),
      }),
      events: ["press"],
      description: "Hamburger that collapses the sidebar.",
    },
    brand: {
      props: z.object({
        title: z.string(),
        subtitle: z.string().nullable(),
      }),
      description: "Product lockup in the topbar.",
    },
    navSection: {
      props: z.object({
        label: z.string(),
        sectionKey: z.string(),
        collapsed: z.boolean().nullable(),
        activeSection: z.string().nullable(),
      }),
      slots: ["default"],
      events: ["toggle"],
      description: "Collapsible sidebar group. Child is a navItemList.",
    },
    navItemList: {
      props: z.object({}),
      slots: ["default"],
      description: "Container for a section's nav items.",
    },
    navItem: {
      props: z.object({
        label: z.string(),
        route: z.string(),
        icon: z.string().nullable(),
        active: z.boolean().nullable(),
        activeRoute: z.string().nullable(),
        aliases: z.array(z.string()).nullable(),
      }),
      events: ["press"],
      description: "Sidebar navigation item. Highlights when route matches activeRoute or aliases.",
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
        subtitle: z.string().nullable(),
        tone: z.enum(["default", "ok", "warn", "danger"]).nullable(),
        className: z.string().nullable(),
      }),
      slots: ["default"],
      description: "Bordered card with optional title, subtitle, and tone.",
    },
    row: {
      props: z.object({
        className: z.string().nullable(),
      }),
      slots: ["default"],
      description: "Horizontal wrapping row. Old-site structural cluster.",
    },
    splitPane: {
      props: z.object({
        direction: z.enum(["vertical", "horizontal"]).nullable(),
        className: z.string().nullable(),
      }),
      slots: ["start", "end"],
      description: "Equal split of two panes, stacked or side-by-side. Accountability-style layout.",
    },
    separator: {
      props: z.object({
        className: z.string().nullable(),
      }),
      description: "Hairline divider.",
    },
    heading: {
      props: z.object({
        text: z.string(),
        level: z.union([z.literal(1), z.literal(2), z.literal(3)]).nullable(),
      }),
      description: "Section heading.",
    },
    code: {
      props: z.object({
        content: z.string(),
      }),
      description: "Preformatted code block.",
    },
    pill: {
      props: z.object({
        text: z.string(),
        tone: z.enum(["default", "ok", "warn", "danger"]).nullable(),
      }),
      description: "Small rounded status label.",
    },
    statusDot: {
      props: z.object({
        status: z.enum(["ok", "warn", "error", "offline"]),
      }),
      description: "Coloured health dot.",
    },
    healthPill: {
      props: z.object({
        label: z.string(),
        okText: z.string(),
        offlineText: z.string(),
      }),
      description: "Live connection pill. Reads /connected.",
    },
    callout: {
      props: z.object({
        tone: z.enum(["default", "ok", "warn", "danger"]).nullable(),
      }),
      slots: ["default"],
      description: "Emphasised inline message block.",
    },
    statusBanner: {
      props: z.object({
        title: z.string().nullable(),
        message: z.string().nullable(),
        tone: z.enum(["default", "ok", "warn", "danger"]).nullable(),
      }),
      description: "Inline banner for backend errors or notices. Hidden when message is empty.",
    },
    emptyState: {
      props: z.object({
        title: z.string(),
        hint: z.string().nullable(),
      }),
      description: "Placeholder shown when a surface has no data.",
    },
    statCard: {
      props: z.object({
        label: z.string(),
        value: z.union([z.string(), z.number()]),
        sub: z.string().nullable(),
        tone: z.enum(["default", "ok", "warn", "danger"]).nullable(),
      }),
      description: "Headline metric card from the old Overview strip.",
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

    // Event-stream objects
    streamObject: {
      props: z.object({
        pluginId: z.string(),
        member: z.string().nullable(),
        className: z.string().nullable(),
      }),
      events: ["press", "select"],
      description:
        "Generic live StateSync object. pluginId selects /plugins/<id>. Optional member shows one field. Emits press/select.",
    },
    streamGrid: {
      props: z.object({
        className: z.string().nullable(),
      }),
      events: ["select"],
      description:
        "Grid of every plugin currently present on the StateSync event stream. Built from /pluginIndex. Emits select with pluginId.",
    },
    catalogIndex: {
      props: z.object({
        className: z.string().nullable(),
      }),
      description:
        "Index of every catalog-registered stream object (camelCase name plus sealed plugin id). Dashboard Catalog equivalent.",
    },

    // antigravity.chat ui_surface — catalog objects for the chat stream
    antigravityChatContainer: {
      props: z.object({
        streamId: z.string(),
        title: z.string().nullable(),
        mode: z.enum(["live", "accountability"]).nullable(),
        placeholder: z.string().nullable(),
      }),
      slots: ["default"],
      description:
        "Host frame for the antigravity.chat ui_surface. Binds live tokens to /streams/<streamId>/messages.",
    },
    chatMessage: {
      props: z.object({
        role: z.enum(["user", "agent", "system"]),
        content: z.string(),
      }),
      description: "Individual text node in the antigravity chat thread.",
    },
    tchedRouterPicker: {
      props: z.object({
        className: z.string().nullable(),
      }),
      description:
        "Provider and model selectors bound to tched_router selected_provider / selected_model and model_routes.",
    },
    antigravityGroupChat: {
      props: z.object({
        streamId: z.string().nullable(),
        title: z.string().nullable(),
        person1Label: z.string().nullable(),
        person2Label: z.string().nullable(),
      }),
      description:
        "2-person group chat with separate identities. Each person has their own model selector and tunnel connection to the remote chatbot.",
    },
    catalogGallery: {
      props: z.object({
        className: z.string().nullable(),
      }),
      description:
        "Self-documenting gallery: every catalog component name rendered with mock props that satisfy its Zod schema.",
    },

    ...streamPluginComponents,
  },
  actions: {
    navigate: {
      params: z.object({ route: z.string() }),
      description: "Navigate the app to a route path.",
    },
    toggleState: {
      params: z.object({ statePath: z.string() }),
      description: "Invert the boolean at a JSON Pointer state path.",
    },
    callMethod: {
      params: z.object({
        subid: z.string(),
        input: z.record(z.string(), z.unknown()),
      }),
      description: "Invoke a plugin method by its OSCAL subid.",
    },
    selectPlugin: {
      params: z.object({ pluginId: z.string() }),
      description: "Record the selected StateSync plugin id in /shell/selectedPlugin.",
    },
  },
});

export type AppCatalog = typeof appCatalog;
export const CATALOG_COMPONENTS = appCatalog.componentNames;
export const CATALOG_ACTIONS = appCatalog.actionNames;
