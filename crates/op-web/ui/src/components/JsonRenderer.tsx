import { useState } from "react";
import { cn } from "@/lib/utils";

interface JsonRendererProps {
  data: unknown;
  defaultMode?: "tree" | "raw";
  maxDepth?: number;
  className?: string;
}

export function JsonRenderer({ data, defaultMode = "tree", maxDepth = 6, className }: JsonRendererProps) {
  const [mode, setMode] = useState<"tree" | "raw">(defaultMode);

  return (
    <div className={cn("rounded-md border border-border bg-card overflow-hidden", className)}>
      <div className="flex items-center justify-between border-b border-border px-3 py-1.5">
        <span className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          Inspector
        </span>
        <div className="flex gap-1">
          {(["tree", "raw"] as const).map((m) => (
            <button
              key={m}
              onClick={() => setMode(m)}
              className={cn(
                "px-2 py-0.5 text-xs rounded-md transition-colors",
                mode === m
                  ? "bg-primary/15 text-primary font-medium"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              {m === "tree" ? "Tree" : "Raw"}
            </button>
          ))}
        </div>
      </div>
      <div className="p-3 overflow-auto max-h-[500px] font-mono text-xs leading-relaxed">
        {mode === "raw" ? (
          <pre className="text-foreground whitespace-pre-wrap break-all">
            {JSON.stringify(data, null, 2)}
          </pre>
        ) : (
          <JsonTree data={data} depth={0} maxDepth={maxDepth} />
        )}
      </div>
    </div>
  );
}

function JsonTree({ data, depth, maxDepth, label }: { data: unknown; depth: number; maxDepth: number; label?: string }) {
  const [collapsed, setCollapsed] = useState(depth > 2);

  if (data === null || data === undefined) {
    return (
      <div className="flex items-baseline gap-1.5">
        {label && <span className="text-muted-foreground">{label}:</span>}
        <span className="text-muted-foreground italic">null</span>
      </div>
    );
  }

  if (typeof data === "boolean") {
    return (
      <div className="flex items-baseline gap-1.5">
        {label && <span className="text-muted-foreground">{label}:</span>}
        <span className={data ? "text-ok" : "text-danger"}>{String(data)}</span>
      </div>
    );
  }

  if (typeof data === "number") {
    return (
      <div className="flex items-baseline gap-1.5">
        {label && <span className="text-muted-foreground">{label}:</span>}
        <span className="text-info">{data}</span>
      </div>
    );
  }

  if (typeof data === "string") {
    return (
      <div className="flex items-baseline gap-1.5 min-w-0">
        {label && <span className="text-muted-foreground shrink-0">{label}:</span>}
        <span className="text-warn break-all">"{data}"</span>
      </div>
    );
  }

  if (Array.isArray(data)) {
    if (depth >= maxDepth) {
      return (
        <div className="flex items-baseline gap-1.5">
          {label && <span className="text-muted-foreground">{label}:</span>}
          <span className="text-muted-foreground">[{data.length} items]</span>
        </div>
      );
    }
    return (
      <div>
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex items-baseline gap-1.5 hover:text-foreground text-muted-foreground"
        >
          <span className="text-[10px]">{collapsed ? "▸" : "▾"}</span>
          {label && <span>{label}:</span>}
          <span className="text-muted-foreground text-[11px]">[{data.length}]</span>
        </button>
        {!collapsed && (
          <div className="ml-4 border-l border-border/50 pl-3 mt-0.5 space-y-0.5">
            {data.map((item, i) => (
              <JsonTree key={i} data={item} depth={depth + 1} maxDepth={maxDepth} label={String(i)} />
            ))}
          </div>
        )}
      </div>
    );
  }

  if (typeof data === "object") {
    const entries = Object.entries(data as Record<string, unknown>);
    if (depth >= maxDepth) {
      return (
        <div className="flex items-baseline gap-1.5">
          {label && <span className="text-muted-foreground">{label}:</span>}
          <span className="text-muted-foreground">{`{${entries.length} keys}`}</span>
        </div>
      );
    }
    return (
      <div>
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex items-baseline gap-1.5 hover:text-foreground text-muted-foreground"
        >
          <span className="text-[10px]">{collapsed ? "▸" : "▾"}</span>
          {label && <span>{label}:</span>}
          <span className="text-muted-foreground text-[11px]">{`{${entries.length}}`}</span>
        </button>
        {!collapsed && (
          <div className="ml-4 border-l border-border/50 pl-3 mt-0.5 space-y-0.5">
            {entries.map(([key, val]) => (
              <JsonTree key={key} data={val} depth={depth + 1} maxDepth={maxDepth} label={key} />
            ))}
          </div>
        )}
      </div>
    );
  }

  return <span className="text-foreground">{String(data)}</span>;
}
