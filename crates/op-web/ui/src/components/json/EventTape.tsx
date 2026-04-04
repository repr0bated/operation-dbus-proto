import { cn } from "@/lib/utils";
import type { SSEEvent } from "@/api/types";
import { Badge } from "@/components/ui/badge";
import { JsonTree } from "@/components/json/JsonTree";
import { useState } from "react";
import { ChevronRight } from "lucide-react";

interface EventTapeProps {
  events: SSEEvent[];
  maxHeight?: string;
  className?: string;
}

const typeColors: Record<string, string> = {
  state_update: "bg-info/20 text-info border-info/30",
  audit_event: "bg-warning/20 text-warning border-warning/30",
  system_stats: "bg-success/20 text-success border-success/30",
  message: "bg-primary/20 text-primary border-primary/30",
  error: "bg-destructive/20 text-destructive border-destructive/30",
};

export function EventTape({ events, maxHeight = "400px", className }: EventTapeProps) {
  return (
    <div className={cn("rounded border bg-card overflow-auto", className)} style={{ maxHeight }}>
      {events.length === 0 ? (
        <div className="p-4 text-xs text-muted-foreground text-center">No events yet</div>
      ) : (
        events.map((event) => <EventRow key={event.id} event={event} />)
      )}
    </div>
  );
}

function EventRow({ event }: { event: SSEEvent }) {
  const [expanded, setExpanded] = useState(false);
  const color = typeColors[event.type] || "bg-muted text-muted-foreground border-border";

  return (
    <div className="border-b border-border/50 last:border-0">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-3 py-1.5 text-left hover:bg-accent/50 transition-colors"
      >
        <ChevronRight className={cn("h-3 w-3 text-muted-foreground shrink-0 transition-transform", expanded && "rotate-90")} />
        <span className="text-[10px] font-mono text-muted-foreground shrink-0 w-20 tabular-nums">
          {new Date(event.timestamp).toLocaleTimeString()}
        </span>
        <span className={cn("inline-flex items-center rounded-sm border px-1.5 py-0 text-[10px] font-medium shrink-0", color)}>
          {event.type}
        </span>
        <span className="text-xs text-muted-foreground truncate flex-1 font-mono">
          {typeof event.data === "object" ? JSON.stringify(event.data).slice(0, 80) : String(event.data)}
        </span>
      </button>
      {expanded && (
        <div className="px-3 pb-2">
          <JsonTree data={event.data} defaultExpanded={true} />
        </div>
      )}
    </div>
  );
}
