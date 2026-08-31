/**
 * ShellRenderer — mounts the shell spec.
 *
 * This is the whole application chrome. It replaces `AppShell.tsx`: the
 * markup now comes from `buildShellSpec()` and the registry, and `children`
 * (the router) is injected into the spec's `contentRegion` slot.
 *
 * Navigation is dynamic: the sidebar includes both static NAV_MANIFEST entries
 * and plugin-contributed routes from ui_surfaces. Use `useDynamicNav` to
 * automatically merge these sources.
 */
import { useMemo, type ReactNode } from "react";
import { Renderer } from "@json-render/react";
import {
  JsonRenderProvider,
  useJsonRender,
} from "@/json-render/runtime/JsonRenderProvider";
import { CONTENT_SLOT } from "@/json-render/runtime/slots";
import { ALLOW_ALL, type CapabilitySet } from "@/json-render/navigation/capabilities";
import { NAV_MANIFEST, type NavItem } from "@/json-render/navigation/manifest";
import { useDynamicNav } from "@/json-render/navigation/use-dynamic-nav";
import { buildShellSpec, DEFAULT_CHROME, type ShellChrome } from "./shellSpec";

function ShellSpecRenderer({ chrome }: { chrome: ShellChrome }) {
  const { registry, capabilities, manifest } = useJsonRender();
  // Capability filtering happens at build time: a filtered-out nav entry is
  // absent from the spec, not merely hidden.
  const spec = useMemo(
    () => buildShellSpec(manifest, capabilities, chrome),
    [manifest, capabilities, chrome],
  );
  return <Renderer spec={spec} registry={registry} />;
}

/**
 * Inner component that uses dynamic navigation.
 * Only uses the hook when no explicit manifest is provided.
 */
function DynamicNavShell({
  children,
  capabilities = ALLOW_ALL,
  chrome = DEFAULT_CHROME,
}: {
  children: ReactNode;
  capabilities?: CapabilitySet;
  chrome?: ShellChrome;
}) {
  // Get dynamic navigation (merges static manifest with plugin ui_surfaces)
  const dynamicNavItems = useDynamicNav();

  return (
    <JsonRenderProvider
      manifest={dynamicNavItems}
      capabilities={capabilities}
      slots={{ [CONTENT_SLOT]: children }}
    >
      <ShellSpecRenderer chrome={chrome} />
    </JsonRenderProvider>
  );
}

/**
 * Static shell component that uses the provided manifest without dynamic nav.
 * Used when an explicit manifest is passed, or for testing.
 */
function StaticShell({
  children,
  manifest,
  capabilities = ALLOW_ALL,
  chrome = DEFAULT_CHROME,
}: {
  children: ReactNode;
  manifest: NavItem[];
  capabilities?: CapabilitySet;
  chrome?: ShellChrome;
}) {
  return (
    <JsonRenderProvider
      manifest={manifest}
      capabilities={capabilities}
      slots={{ [CONTENT_SLOT]: children }}
    >
      <ShellSpecRenderer chrome={chrome} />
    </JsonRenderProvider>
  );
}

export function ShellRenderer({
  children,
  manifest,
  capabilities = ALLOW_ALL,
  chrome = DEFAULT_CHROME,
}: {
  children: ReactNode;
  /** Pass explicit manifest to disable dynamic nav. Defaults to useDynamicNav(). */
  manifest?: NavItem[];
  capabilities?: CapabilitySet;
  chrome?: ShellChrome;
}) {
  // When an explicit manifest is provided, use it directly without dynamic nav.
  // This avoids unnecessary hook calls and test complications.
  if (manifest) {
    return (
      <StaticShell
        manifest={manifest}
        capabilities={capabilities}
        chrome={chrome}
      >
        {children}
      </StaticShell>
    );
  }

  // Default: use dynamic navigation
  return (
    <DynamicNavShell capabilities={capabilities} chrome={chrome}>
      {children}
    </DynamicNavShell>
  );
}
