import { useEffect, useMemo, useRef } from "react";
import {
  JSONUIProvider,
  Renderer,
  type SetState,
} from "@json-render/react";
import { useEventStream } from "./stream/use-event-stream";
import { uiStore } from "./store/ui-store";
import { activeSectionSlug } from "./navigation/manifest";
import { shellSpec } from "./specs/shell";
import { registry, handlers as catalogHandlers } from "./catalog/registry";

export function App() {
  useEventStream();

  useEffect(() => {
    const onPop = () => {
      const path = window.location.pathname || "/";
      uiStore.set("/shell/route", path);
      uiStore.set("/shell/activeSection", activeSectionSlug(path));
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  const snapshotRef = useRef(uiStore.getSnapshot);
  snapshotRef.current = uiStore.getSnapshot;

  const setState = useRef<SetState>((updater) => {
    const next = updater(snapshotRef.current());
    const updates: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(next)) {
      if (key === "shell" && value && typeof value === "object") {
        for (const [subkey, subvalue] of Object.entries(value as Record<string, unknown>)) {
          updates[`/shell/${subkey}`] = subvalue;
        }
      } else {
        updates[`/${key}`] = value;
      }
    }
    uiStore.update(updates);
  });

  const handlers = useMemo(
    () => catalogHandlers(() => setState.current, () => snapshotRef.current()),
    [],
  );

  return (
    <JSONUIProvider registry={registry} store={uiStore} handlers={handlers}>
      <div className="min-h-screen">
        <Renderer spec={shellSpec} registry={registry} />
      </div>
    </JSONUIProvider>
  );
}
