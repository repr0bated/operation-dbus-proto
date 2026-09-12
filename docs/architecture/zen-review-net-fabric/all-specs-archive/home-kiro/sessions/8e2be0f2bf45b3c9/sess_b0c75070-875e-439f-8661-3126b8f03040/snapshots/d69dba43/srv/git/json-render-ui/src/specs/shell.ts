import type { Spec } from "@json-render/core";

export const shellSpec: Spec = {
  root: "shell",
  elements: {
    shell: {
      type: "shell",
      props: { navCollapsed: false },
      slots: {
        nav: ["nav-overview", "nav-network", "nav-plugins", "nav-chat"],
        header: ["header"],
        content: ["overview"],
      },
    },
    "nav-overview": {
      type: "navItem",
      props: { label: "Overview", route: "/", icon: "home", active: true },
      on: { press: { action: "navigate", params: { route: "/" } } },
    },
    "nav-network": {
      type: "navItem",
      props: { label: "Network", route: "/network", icon: "globe", active: false },
      on: { press: { action: "navigate", params: { route: "/network" } } },
    },
    "nav-plugins": {
      type: "navItem",
      props: { label: "Plugins", route: "/plugins", icon: "puzzle", active: false },
      on: { press: { action: "navigate", params: { route: "/plugins" } } },
    },
    "nav-chat": {
      type: "navItem",
      props: { label: "Chat", route: "/chat", icon: "message", active: false },
      on: { press: { action: "navigate", params: { route: "/chat" } } },
    },
    header: {
      type: "pageHeader",
      props: { title: "Overview", subtitle: "Live system state from the event stream" },
    },
    overview: {
      type: "container",
      props: { className: "flex flex-col gap-6" },
      children: ["stats-row", "plugins-card"],
    },
    "stats-row": {
      type: "grid",
      props: { cols: 4, gap: 16 },
      children: ["stat-plugins", "stat-events", "stat-uptime", "stat-blobs"],
    },
    "stat-plugins": {
      type: "stat",
      props: { label: "Plugins", value: "66", unit: "sealed" },
    },
    "stat-events": {
      type: "stateValue",
      props: { path: "snowball.event_count", label: "Events", format: "raw" },
    },
    "stat-uptime": {
      type: "stateValue",
      props: { path: "procfs.uptime", label: "Uptime", format: "duration" },
    },
    "stat-blobs": {
      type: "stateValue",
      props: { path: "full_system.blob_count", label: "Blobs", format: "raw" },
    },
    "plugins-card": {
      type: "card",
      props: { title: "Active Plugins" },
      children: ["plugins-list"],
    },
    "plugins-list": {
      type: "stateValue",
      props: { path: "full_system.active_plugins", label: null, format: "json" },
    },
  },
};
