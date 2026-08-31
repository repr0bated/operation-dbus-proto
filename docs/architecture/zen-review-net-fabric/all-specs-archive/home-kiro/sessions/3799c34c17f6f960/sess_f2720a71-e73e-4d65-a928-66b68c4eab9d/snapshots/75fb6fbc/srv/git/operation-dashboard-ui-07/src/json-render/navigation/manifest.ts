/**
 * Navigation manifest — single source of truth for sidebar entries.
 *
 * Icons are stored as string keys resolved by `iconRegistry` to keep the
 * manifest serialisable (so it can be shipped alongside JSON specs later).
 * The current `AppShell` iterates this manifest instead of hard-coded
 * arrays; nothing else about the visual layout changed.
 */

import type { Capability } from "./capabilities";

/**
 * Derived from `SECTION_ORDER`, which is the only place sections are declared.
 *
 * This used to be a second list — a union type alongside the order array — and
 * the two drifted: a section named by the type but missing from the order had
 * all of its entries dropped from the sidebar by `buildNavGroups`, silently.
 * Deriving the type means a section the order does not list cannot be named by a
 * manifest entry at all.
 */
export type NavSection = (typeof SECTION_ORDER)[number];

export interface NavItem {
  id: string;
  label: string;
  route: string;
  icon: string;
  section: NavSection;
  order: number;
  requiredCapabilities?: Capability[];
  /** Not implemented at all — nav entry is disabled. */
  placeholder?: boolean;
  /**
   * Work in progress: the page renders but is not wired to live control-plane
   * data. Set this ONLY for pages that show static/invented content — a page
   * that calls a real backend and fails is *not* WIP, because that failure is
   * useful signal and must stay visible.
   *
   * Marking this drives both the sidebar badge and the on-page banner; no
   * per-page edit is required.
   */
  wip?: boolean;
  /** Shown in the WIP banner so the gap is explicit. */
  wipReason?: string;
  /** Additional pathnames that should highlight this nav entry. */
  aliases?: string[];
}

export const NAV_MANIFEST: NavItem[] = [
  {
    id: "chat",
    label: "Chat",
    route: "/antigravity/chat",
    icon: "MessageSquare",
    section: "Chat",
    order: 10,
    aliases: ["/chat", "/antigravity"],
  },
  { id: "accountability", label: "Accountability", route: "/accountability", icon: "ScrollText", section: "Chat", order: 20 },
  { id: "catalog", label: "Catalog", route: "/catalog", icon: "FileText", section: "UI Model", order: 10 },
  { id: "gallery", label: "Gallery", route: "/gallery", icon: "Sparkles", section: "UI Model", order: 20 },
  { id: "generate", label: "Generate", route: "/generate", icon: "Sparkles", section: "UI Model", order: 30 },
  { id: "overview", label: "Overview", route: "/", icon: "BarChart3", section: "Control", order: 10 },
  { id: "plugins", label: "Stream", route: "/plugins", icon: "Package", section: "Infrastructure", order: 10 },
  { id: "network", label: "Network", route: "/network", icon: "Network", section: "Infrastructure", order: 20 },
];

/**
 * Every section, in sidebar order. Adding a section here is what makes it
 * nameable by a manifest entry (see [`NavSection`]); nothing else needs editing.
 */
export const SECTION_ORDER = [
  "Chat",
  "UI Model",
  "Control",
  "Infrastructure",
  "Plugins",
] as const;

export interface NavGroup {
  section: NavSection;
  items: NavItem[];
}

import { hasAll, type CapabilitySet } from "./capabilities";

/**
 * Stable, pointer-safe slug for a section label.
 *
 * Section labels are human strings ("UI Model", "Coming Soon") but they are
 * also used as JSON Pointer segments in the shell state
 * (`/shell/collapsedSections/<slug>`), so they must not contain spaces.
 */
export function sectionSlug(section: NavSection | string): string {
  return section
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

/** Slug of the section that owns `route`, or "" when none matches. */
export function activeSectionSlug(
  route: string,
  manifest: NavItem[] = NAV_MANIFEST,
): string {
  // Exact match wins; "/" must not be treated as a prefix of everything.
  const exact = manifest.find(
    (i) => i.route === route || i.aliases?.includes(route),
  );
  if (exact) return sectionSlug(exact.section);
  const prefixed = manifest.find(
    (i) => i.route !== "/" && route.startsWith(i.route),
  );
  return prefixed ? sectionSlug(prefixed.section) : "";
}

/** Filter + group manifest entries into ordered sections. */
export function buildNavGroups(
  manifest: NavItem[],
  caps: CapabilitySet,
): NavGroup[] {
  const bySection = new Map<NavSection, NavItem[]>();
  for (const item of manifest) {
    if (!hasAll(caps, item.requiredCapabilities)) continue;
    const bucket = bySection.get(item.section) ?? [];
    bucket.push(item);
    bySection.set(item.section, bucket);
  }
  return SECTION_ORDER.filter((s) => bySection.has(s)).map((section) => ({
    section,
    items: (bySection.get(section) ?? []).sort((a, b) => a.order - b.order),
  }));
}
