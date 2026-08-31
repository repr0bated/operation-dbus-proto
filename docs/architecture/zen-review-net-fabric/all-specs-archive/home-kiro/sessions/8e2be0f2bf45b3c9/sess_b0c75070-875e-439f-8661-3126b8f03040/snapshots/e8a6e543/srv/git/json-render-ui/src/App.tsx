import { Renderer } from "@json-render/react";
import { useStateStore } from "./store/state-store";
import { useEventStream } from "./stream/use-event-stream";
import { shellSpec } from "./specs/shell";
import { registry } from "./catalog/registry";

export function App() {
  useEventStream();
  const connected = useStateStore((s) => s.connected);

  return (
    <div className="min-h-screen">
      {!connected && (
        <div className="fixed top-0 inset-x-0 z-50 bg-amber-900/80 text-amber-100 text-xs text-center py-1">
          Connecting to event stream...
        </div>
      )}
      <Renderer spec={shellSpec} registry={registry} />
    </div>
  );
}
