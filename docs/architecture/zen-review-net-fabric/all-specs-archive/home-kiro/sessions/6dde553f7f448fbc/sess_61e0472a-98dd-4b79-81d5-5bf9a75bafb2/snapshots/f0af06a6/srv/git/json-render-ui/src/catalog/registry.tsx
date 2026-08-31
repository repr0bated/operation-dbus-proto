/**
 * The registry: catalog name → implementation.
 *
 * `defineRegistry` type-checks this map against the catalog, so a component
 * declared in the catalog with no implementation (or a typo in either place)
 * is a compile error rather than a blank region at runtime.
 */
import { defineRegistry } from "@json-render/react";
import { appCatalog } from "./catalog";
import { STREAM_PLUGIN_IDS, toCatalogName } from "./stream-plugins";
import { Shell } from "./components/shell";
import { NavItem, NavSection, NavItemList, NavToggle, Brand, Topbar, TopbarGroup } from "./components/nav";
import { PageHeader } from "./components/page-header";
import { Container, Grid, Card } from "./components/layout";
import { Kv, Text, Badge, Stat } from "./components/data";
import {
  Row,
  SplitPane,
  Separator,
  Heading,
  Code,
  Pill,
  StatusDot,
  HealthPill,
  Callout,
  StatusBanner,
  EmptyState,
  StatCard,
} from "./components/structure";
import { PluginState, StateValue } from "./components/live";
import { StreamObjectEl, StreamGridEl, makeStreamPluginEl } from "./components/stream-object";
import { CatalogIndexEl } from "./components/catalog-index";
import { AntigravityChatContainerEl, ChatMessageEl } from "./components/antigravity-chat";
import { AntigravityGroupChatEl } from "./components/antigravity-group-chat";
import { TchedRouterPickerEl } from "./components/tched-router-picker";
import { CatalogGalleryEl } from "./components/catalog-gallery";
import { uiStore } from "@/store/ui-store";
import { activeSectionSlug } from "@/navigation/manifest";
import { setAppRegistry } from "./registry-holder";

const namedStreamComponents = Object.fromEntries(
  STREAM_PLUGIN_IDS.map((id) => [toCatalogName(id), makeStreamPluginEl(id)]),
);

const defined = defineRegistry(appCatalog, {
  components: {
    shell: Shell,
    topbar: Topbar,
    topbarGroup: TopbarGroup,
    navToggle: NavToggle,
    brand: Brand,
    navSection: NavSection,
    navItemList: NavItemList,
    navItem: NavItem,
    pageHeader: PageHeader,
    container: Container,
    grid: Grid,
    card: Card,
    row: Row,
    splitPane: SplitPane,
    separator: Separator,
    heading: Heading,
    code: Code,
    pill: Pill,
    statusDot: StatusDot,
    healthPill: HealthPill,
    callout: Callout,
    statusBanner: StatusBanner,
    emptyState: EmptyState,
    statCard: StatCard,
    kv: Kv,
    text: Text,
    badge: Badge,
    stat: Stat,
    pluginState: PluginState,
    stateValue: StateValue,
    streamObject: StreamObjectEl,
    streamGrid: StreamGridEl,
    catalogIndex: CatalogIndexEl,
    antigravityChatContainer: AntigravityChatContainerEl,
    chatMessage: ChatMessageEl,
    antigravityGroupChat: AntigravityGroupChatEl,
    tchedRouterPicker: TchedRouterPickerEl,
    catalogGallery: CatalogGalleryEl,
    ...namedStreamComponents,
  },
  actions: {
    navigate: async (params, setState) => {
      const route = params?.route;
      if (!route) return;
      if (typeof window !== "undefined" && window.history) {
        window.history.pushState({}, "", route);
      }
      setState((prev) => {
        const next: Record<string, unknown> = { ...prev };
        const shell = { ...((prev.shell as Record<string, unknown> | undefined) ?? {}) };
        shell.route = route;
        shell.activeSection = activeSectionSlug(route);
        next.shell = shell;
        return next;
      });
    },
    toggleState: async (params) => {
      const path = params?.statePath;
      if (typeof path !== "string" || !path) return;
      uiStore.set(path, !uiStore.get(path));
    },
    callMethod: async (params) => {
      console.log("callMethod:", params?.subid, params?.input);
    },
    selectPlugin: async (params, setState) => {
      const pluginId = params?.pluginId;
      if (!pluginId) return;
      setState((prev) => {
        const next: Record<string, unknown> = { ...prev };
        const shell = { ...((prev.shell as Record<string, unknown> | undefined) ?? {}) };
        shell.selectedPlugin = pluginId;
        next.shell = shell;
        return next;
      });
    },
  },
});

setAppRegistry(defined.registry);
export const { registry, handlers, executeAction } = defined;
