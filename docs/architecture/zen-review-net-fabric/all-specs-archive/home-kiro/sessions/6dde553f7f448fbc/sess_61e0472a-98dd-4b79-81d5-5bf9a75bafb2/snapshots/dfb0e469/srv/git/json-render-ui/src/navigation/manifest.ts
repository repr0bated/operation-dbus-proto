export const SECTION_ORDER = ["Chat", "UI Model", "Control", "Infrastructure"] as const;
export type NavSection = (typeof SECTION_ORDER)[number];

export interface NavManifestItem {
  id: string;
  label: string;
  route: string;
  icon: string;
  section: NavSection;
  order: number;
  aliases?: string[];
}

export const NAV_MANIFEST: NavManifestItem[] = [
  { id: "chat", label: "Chat", route: "/antigravity/chat", icon: "message", section: "Chat", order: 10, aliases: ["/chat", "/antigravity"] },
  { id: "accountability", label: "Accountability", route: "/accountability", icon: "message", section: "Chat", order: 20 },
  { id: "catalog", label: "Catalog", route: "/catalog", icon: "file", section: "UI Model", order: 10 },
  { id: "gallery", label: "Gallery", route: "/gallery", icon: "file", section: "UI Model", order: 20 },
  { id: "overview", label: "Overview", route: "/", icon: "home", section: "Control", order: 10 },
  { id: "plugins", label: "Stream", route: "/plugins", icon: "puzzle", section: "Infrastructure", order: 10 },
  { id: "network", label: "Network", route: "/network", icon: "globe", section: "Infrastructure", order: 20 },
];

export function sectionSlug(section: string): string {
  return section.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

export function activeSectionSlug(route: string, manifest: NavManifestItem[] = NAV_MANIFEST): string {
  const exact = manifest.find((item) => item.route === route || item.aliases?.includes(route));
  if (exact) return sectionSlug(exact.section);
  const prefixed = manifest.find((item) => item.route !== "/" && route.startsWith(item.route));
  return prefixed ? sectionSlug(prefixed.section) : "";
}

export function buildNavGroups(manifest: NavManifestItem[] = NAV_MANIFEST) {
  const bySection = new Map<NavSection, NavManifestItem[]>();
  for (const item of manifest) {
    const bucket = bySection.get(item.section) ?? [];
    bucket.push(item);
    bySection.set(item.section, bucket);
  }
  return SECTION_ORDER.filter((section) => bySection.has(section)).map((section) => ({
    section,
    items: (bySection.get(section) ?? []).sort((a, b) => a.order - b.order),
  }));
}
