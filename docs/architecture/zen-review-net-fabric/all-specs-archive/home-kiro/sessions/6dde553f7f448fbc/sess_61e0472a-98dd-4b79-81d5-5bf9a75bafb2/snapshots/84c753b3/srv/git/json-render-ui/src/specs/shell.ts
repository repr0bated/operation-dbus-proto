import type { Spec, UIElement } from "@json-render/core";
import { buildNavGroups, sectionSlug } from "@/navigation/manifest";

const SHELL = {
  navCollapsed: "/shell/navCollapsed",
  route: "/shell/route",
  activeSection: "/shell/activeSection",
  collapsedSection: (slug: string) => `/shell/collapsedSections/${slug}`,
} as const;

function navElements(): Record<string, UIElement> {
  const elements: Record<string, UIElement> = {};
  const navKeys: string[] = [];

  for (const group of buildNavGroups()) {
    const slug = sectionSlug(group.section);
    const sectionKey = `nav-section-${slug}`;
    const listKey = `nav-list-${slug}`;
    const itemKeys = group.items.map((item) => {
      const key = `nav-item-${item.id}`;
      elements[key] = {
        type: "navItem",
        props: {
          label: item.label,
          route: item.route,
          icon: item.icon,
          active: null,
          activeRoute: { $state: SHELL.route },
          aliases: item.aliases ?? null,
        },
        on: { press: { action: "navigate", params: { route: item.route } } },
      };
      return key;
    });
    elements[listKey] = {
      type: "navItemList",
      props: {},
      children: itemKeys,
      visible: {
        $or: [
          { $state: SHELL.collapsedSection(slug), not: true },
          { $state: SHELL.activeSection, eq: slug },
        ],
      },
    };
    elements[sectionKey] = {
      type: "navSection",
      props: {
        label: group.section,
        sectionKey: slug,
        collapsed: { $state: SHELL.collapsedSection(slug) },
        activeSection: { $state: SHELL.activeSection },
      },
      children: [listKey],
      on: {
        toggle: { action: "toggleState", params: { statePath: SHELL.collapsedSection(slug) } },
      },
    };
    navKeys.push(sectionKey);
  }

  elements["topbar-left"] = {
    type: "topbarGroup",
    props: { align: "start" },
    children: ["nav-toggle", "brand"],
  };
  elements["nav-toggle"] = {
    type: "navToggle",
    props: {
      collapsed: { $state: SHELL.navCollapsed },
      expandTitle: "Expand sidebar",
      collapseTitle: "Collapse sidebar",
    },
    on: { press: { action: "toggleState", params: { statePath: SHELL.navCollapsed } } },
  };
  elements.brand = {
    type: "brand",
    props: { title: "3tched", subtitle: "json-render" },
  };
  elements["topbar-right"] = {
    type: "topbarGroup",
    props: { align: "end" },
    children: ["health-pill"],
  };
  elements["health-pill"] = {
    type: "healthPill",
    props: { label: "Health", okText: "OK", offlineText: "Offline" },
  };
  elements.topbar = {
    type: "topbar",
    props: {},
    children: ["topbar-left", "topbar-right"],
  };
  elements.shell = {
    type: "shell",
    props: {
      navCollapsed: { $state: SHELL.navCollapsed },
      navWidth: "220px",
      collapsedNavWidth: "0px",
      topbarHeight: "56px",
    },
    slots: {
      topbar: ["topbar"],
      nav: navKeys,
      content: ["overview", "catalog-page", "gallery-page", "plugins-page", "network-page", "chat-page"],
    },
  };
  return elements;
}

const pageElements: Record<string, UIElement> = {
  overview: {
    type: "container",
    props: { className: "flex flex-col gap-6" },
    visible: { $state: SHELL.route, eq: "/" },
    children: ["overview-header", "overview-health", "overview-banner", "stats-row", "stream-live"],
  },
  "overview-header": {
    type: "pageHeader",
    props: {
      title: "Overview",
      subtitle: "System status and live StateSync objects",
    },
  },
  "overview-health": {
    type: "row",
    props: { className: null },
    children: ["overview-health-pill"],
  },
  "overview-health-pill": {
    type: "healthPill",
    props: { label: "StateSync", okText: "Connected", offlineText: "Disconnected" },
  },
  "overview-banner": {
    type: "statusBanner",
    props: {
      title: "Event stream",
      message: {
        $cond: { $state: "/connected" },
        $then: null,
        $else: "Disconnected from StateSync",
      },
      tone: "warn",
    },
  },
  "stats-row": {
    type: "grid",
    props: { cols: 4, gap: 16, className: null },
    children: ["stat-plugins", "stat-events", "stat-uptime", "stat-blobs"],
  },
  "stat-plugins": {
    type: "statCard",
    props: { label: "Plugins", value: "66", sub: "sealed", tone: "default" },
  },
  "stat-events": {
    type: "stateValue",
    props: { path: "blockchain.event_count", label: "Events", format: "raw" },
  },
  "stat-uptime": {
    type: "stateValue",
    props: { path: "procfs.uptime", label: "Uptime", format: "duration" },
  },
  "stat-blobs": {
    type: "stateValue",
    props: { path: "full_system.blob_count", label: "Blobs", format: "raw" },
  },
  "stream-live": {
    type: "card",
    props: { title: "Live stream objects", subtitle: "Every sealed plugin on the event stream", tone: null, className: null },
    children: ["stream-grid-overview"],
  },
  "stream-grid-overview": {
    type: "streamGrid",
    props: { className: null },
    on: { select: { action: "selectPlugin", params: { pluginId: { $state: "/shell/selectedPlugin" } } } },
  },
  "catalog-page": {
    type: "container",
    props: { className: "flex flex-col gap-4" },
    visible: { $state: SHELL.route, eq: "/catalog" },
    children: ["catalog-header", "catalog-index", "catalog-live"],
  },
  "catalog-header": {
    type: "pageHeader",
    props: {
      title: "Catalog",
      subtitle: "Registered json-render UI objects for every StateSync stream plugin",
    },
  },
  "catalog-index": {
    type: "catalogIndex",
    props: { className: null },
  },
  "catalog-live": {
    type: "card",
    props: { title: "Live stream objects", subtitle: null, tone: null, className: null },
    children: ["catalog-stream-grid"],
  },
  "catalog-stream-grid": {
    type: "streamGrid",
    props: { className: null },
    on: { select: { action: "selectPlugin", params: { pluginId: { $state: "/shell/selectedPlugin" } } } },
  },
  "gallery-page": {
    type: "container",
    props: { className: "flex flex-col gap-4" },
    visible: { $state: SHELL.route, eq: "/gallery" },
    children: ["gallery-header", "gallery-body"],
  },
  "gallery-header": {
    type: "pageHeader",
    props: {
      title: "Gallery",
      subtitle: "Every catalog component rendered with mock props that satisfy its Zod schema",
    },
  },
  "gallery-body": {
    type: "catalogGallery",
    props: { className: null },
  },
  "plugins-page": {
    type: "container",
    props: { className: "flex flex-col gap-4" },
    visible: { $state: SHELL.route, eq: "/plugins" },
    children: ["plugins-header", "stream-grid"],
  },
  "plugins-header": {
    type: "pageHeader",
    props: {
      title: "Event stream",
      subtitle: "Registered catalog objects for every StateSync plugin",
    },
  },
  "stream-grid": {
    type: "streamGrid",
    props: { className: null },
    on: { select: { action: "selectPlugin", params: { pluginId: { $state: "/shell/selectedPlugin" } } } },
  },
  "network-page": {
    type: "container",
    props: { className: "flex flex-col gap-4" },
    visible: { $state: SHELL.route, eq: "/network" },
    children: ["net-header", "net-object"],
  },
  "net-header": {
    type: "pageHeader",
    props: { title: "Network", subtitle: "Named catalog object: net" },
  },
  "net-object": {
    type: "net",
    props: { member: null, className: null },
    on: { press: { action: "selectPlugin", params: { pluginId: "net" } } },
  },
  "adc-page": {
    type: "container",
    props: { className: "flex flex-col gap-4" },
    visible: { $state: SHELL.route, eq: "/adc" },
    children: ["adc-header", "adc-object"],
  },
  "adc-header": {
    type: "pageHeader",
    props: { title: "ADC", subtitle: "Named catalog object: adc" },
  },
  "adc-object": {
    type: "adc",
    props: { member: null, className: null },
    on: { press: { action: "selectPlugin", params: { pluginId: "adc" } } },
  },
  "agent-config-page": {
    type: "container",
    props: { className: "flex flex-col gap-4" },
    visible: { $state: SHELL.route, eq: "/agent-config" },
    children: ["agent-config-header", "agent-config-object"],
  },
  "agent-config-header": {
    type: "pageHeader",
    props: { title: "Agent Config", subtitle: "Named catalog object: agent_config" },
  },
  "agent-config-object": {
    type: "agenConfig",
    props: { member: null, className: null },
    on: { press: { action: "selectPlugin", params: { pluginId: "agent_config" } } },
  },
  "chat-page": {
    type: "container",
    props: { className: "flex flex-col gap-3 h-[calc(100vh-7rem)] min-h-0" },
    visible: {
      $or: [
        { $state: SHELL.route, eq: "/antigravity/chat" },
        { $state: SHELL.route, eq: "/antigravity" },
        { $state: SHELL.route, eq: "/chat" },
        { $state: SHELL.route, eq: "/accountability" },
      ],
    },
    children: ["chat-header", "tched-picker", "chat-stack"],
  },
  "chat-stack": {
    type: "splitPane",
    props: { direction: "vertical", className: "flex-1 min-h-0" },
    slots: {
      start: ["antigravity-chat"],
      end: ["accountability-chat"],
    },
  },
  "chat-header": {
    type: "pageHeader",
    props: {
      title: "Antigravity Chat · Accountability",
      subtitle: "Stacked panes · live Send on top · ctl_plane_chatbot episodes below · model from tched_router",
    },
  },
  "tched-picker": {
    type: "tchedRouterPicker",
    props: { className: null },
  },
  "antigravity-chat": {
    type: "antigravityChatContainer",
    props: {
      streamId: "stream_antigravity_01",
      title: "Antigravity Chat",
      mode: "live",
      placeholder: "Send antigravity chat…",
    },
    children: ["chat-thread"],
  },
  "chat-thread": {
    type: "container",
    props: { className: "flex flex-col gap-2" },
    repeat: { statePath: "/streams/stream_antigravity_01/messages", key: "id" },
    children: ["chat-message"],
  },
  "accountability-chat": {
    type: "antigravityChatContainer",
    props: {
      streamId: "stream_accountability_01",
      title: "Accountability Chat",
      mode: "accountability",
      placeholder: "Ask about reasoning episodes…",
    },
    children: ["acct-thread"],
  },
  "acct-thread": {
    type: "container",
    props: { className: "flex flex-col gap-2" },
    repeat: { statePath: "/streams/stream_accountability_01/messages", key: "id" },
    children: ["acct-message"],
  },
  "chat-message": {
    type: "chatMessage",
    props: {
      role: { $item: "role" },
      content: { $item: "content" },
    },
  },
  "acct-message": {
    type: "chatMessage",
    props: {
      role: { $item: "role" },
      content: { $item: "content" },
    },
  },
};

export const shellSpec: Spec = {
  root: "shell",
  elements: {
    ...navElements(),
    ...pageElements,
  },
};
