import { useMemo } from "react";
import { Renderer, StateProvider, VisibilityProvider, ActionProvider } from "@json-render/react";
import type { ActionHandler } from "@json-render/core";
import { useStateStore } from "./store/state-store";
import { useEventStream } from "./stream/use-event-stream";
import { shellSpec } from "./specs/shell";
import { registry } from "./catalog/registry";

const ACTION_HANDLERS: Record<string, ActionHandler> = {
  navigate: (params) => {
    const route = (params as Record<string, string>).route;
    console.log("navigate:", route);
    // TODO: update active nav state + swap content spec
  },
  callMethod: async (params) => {
    const { subid, input } = params as { subid: string; input: Record<string, unknown> };
    console.log("callMethod:", subid, input);
  },
};

export function App() {
  useEventStream();
  const connected = useStateStore((s) => s.connected);
  const handlers = useMemo(() => ACTION_HANDLERS, []);

  return (
    <StateProvider>
      <VisibilityProvider>
        <ActionProvider handlers={handlers}>
          <div className="min-h-screen">
            {!connected && (
              <div className="fixed top-0 inset-x-0 z-50 bg-amber-900/80 text-amber-100 text-xs text-center py-1">
                Connecting to event stream...
              </div>
            )}
            <Renderer spec={shellSpec} registry={registry} />
          </div>
        </ActionProvider>
      </VisibilityProvider>
    </StateProvider>
  );
}
