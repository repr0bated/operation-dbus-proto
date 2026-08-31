/**
 * ShellRenderer — mounts the shell spec.
 *
 * This is the whole application chrome. It replaces `AppShell.tsx`: the
 * markup now comes from `buildShellSpec()` and the registry, and `children`
 * (the router) is injected into the spec's `contentRegion` slot.
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
import { buildShellSpec, DEFAULT_CHROME, type ShellChrome } from "./shellSpec";

function ShellSpecRenderer({
  manifest,
  chrome,
}: {
  manifest: NavItem[];
  chrome: ShellChrome;
}) {
  const { registry, capabilities } = useJsonRender();
  // Capability filtering happens at build time: a filtered-out nav entry is
  // absent from the spec, not merely hidden.
  const spec = useMemo(
    () => buildShellSpec(manifest, capabilities, chrome),
    [manifest, capabilities, chrome],
  );
  return <Renderer spec={spec} registry={registry} />;
}

export function ShellRenderer({
  children,
  manifest = NAV_MANIFEST,
  capabilities = ALLOW_ALL,
  chrome = DEFAULT_CHROME,
}: {
  children: ReactNode;
  manifest?: NavItem[];
  capabilities?: CapabilitySet;
  chrome?: ShellChrome;
}) {
  return (
    <JsonRenderProvider
      manifest={manifest}
      capabilities={capabilities}
      slots={{ [CONTENT_SLOT]: children }}
    >
      <ShellSpecRenderer manifest={manifest} chrome={chrome} />
    </JsonRenderProvider>
  );
}
