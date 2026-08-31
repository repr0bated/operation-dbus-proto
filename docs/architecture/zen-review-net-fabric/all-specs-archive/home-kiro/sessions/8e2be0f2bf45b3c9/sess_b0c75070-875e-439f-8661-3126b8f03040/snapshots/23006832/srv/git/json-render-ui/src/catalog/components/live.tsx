import type { ReactNode } from "react";
import { useStateStore } from "@/store/state-store";

interface PluginStateRenderProps {
  element: { props: { pluginId: string } };
  children?: ReactNode;
}

export function PluginState({ element, children }: PluginStateRenderProps) {
  const state = useStateStore((s) => s.plugins[element.props.pluginId]);
  if (!state) {
    return (
      <div className="text-xs text-neutral-500 italic">
        waiting for {element.props.pluginId}...
      </div>
    );
  }
  return <>{children}</>;
}

interface StateValueRenderProps {
  element: {
    props: {
      path: string;
      label?: string | null;
      format?: "raw" | "json" | "bytes" | "duration" | null;
    };
  };
}

export function StateValue({ element }: StateValueRenderProps) {
  const { path, label, format } = element.props;
  const value = useStateStore((s) => {
    const parts = path.split(".");
    let current: unknown = s.plugins;
    for (const part of parts) {
      if (current === null || current === undefined || typeof current !== "object") return undefined;
      current = (current as Record<string, unknown>)[part];
    }
    return current;
  });

  const display =
    value === null || value === undefined
      ? "\u2014"
      : format === "json"
        ? JSON.stringify(value, null, 2)
        : format === "bytes" && typeof value === "number"
          ? formatBytes(value)
          : String(value);

  return (
    <div className="flex items-baseline justify-between py-1 text-sm">
      {label && <span className="text-neutral-400">{label}</span>}
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
