import type { ReactNode } from "react";
import { useStateStore } from "@/store/state-store";

interface PluginStateProps {
  props: { pluginId: string };
  children?: ReactNode;
}

export function PluginState({ props, children }: PluginStateProps) {
  const state = useStateStore((s) => s.plugins[props.pluginId]);
  if (!state) {
    return (
      <div className="text-xs text-neutral-500 italic">
        waiting for {props.pluginId}...
      </div>
    );
  }
  return <>{children}</>;
}

interface StateValueProps {
  props: {
    path: string;
    label?: string | null;
    format?: "raw" | "json" | "bytes" | "duration" | null;
  };
}

export function StateValue({ props }: StateValueProps) {
  const value = useStateStore((s) => {
    const parts = props.path.split(".");
    let current: unknown = s.plugins;
    for (const part of parts) {
      if (current === null || current === undefined || typeof current !== "object") return undefined;
      current = (current as Record<string, unknown>)[part];
    }
    return current;
  });

  const display =
    value === null || value === undefined
      ? "—"
      : props.format === "json"
        ? JSON.stringify(value, null, 2)
        : props.format === "bytes" && typeof value === "number"
          ? formatBytes(value)
          : String(value);

  return (
    <div className="flex items-baseline justify-between py-1 text-sm">
      {props.label && <span className="text-neutral-400">{props.label}</span>}
      <span className="text-neutral-100 font-mono text-xs whitespace-pre-wrap">{display}</span>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
