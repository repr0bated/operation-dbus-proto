import { useEventStream } from "@/api/eventStream";
import type { StateEntry } from "@/api/types";
import { JsonTree } from "@/components/json/JsonTree";
import { cn } from "@/lib/utils";

interface StateProjectionPanelProps {
  className?: string;
}

export function StateProjectionPanel({ className }: StateProjectionPanelProps) {
  const latestState = useEventStream((s) => s.latestState);
  const entries = Array.from(latestState.values());

  if (entries.length === 0) {
    return (
      <div className={cn("rounded border bg-card p-4 text-xs text-muted-foreground text-center", className)}>
        No state projections yet
      </div>
    );
  }

  return (
    <div className={cn("rounded border bg-card divide-y", className)}>
      {entries.map((entry) => (
        <div key={entry.key} className="p-2 space-y-1">
          <div className="flex items-center justify-between">
            <span className="font-mono text-xs font-medium">{entry.key}</span>
            <span className="text-[10px] text-muted-foreground">{entry.plugin} · {entry.object_type}</span>
          </div>
          <JsonTree data={entry.value} defaultExpanded={false} />
          <div className="text-[10px] text-muted-foreground">
            Updated {new Date(entry.updated_at).toLocaleTimeString()}
          </div>
        </div>
      ))}
    </div>
  );
}
