import { cn } from "@/lib/utils";

interface EventDistributionProps {
  counts: Record<string, number>;
  className?: string;
}

const EVENT_COLORS: Record<string, string> = {
  health: "bg-ok",
  log: "bg-muted-foreground",
  state_update: "bg-info",
  audit_event: "bg-warn",
  system_stats: "bg-primary",
  message: "bg-accent",
  service_change: "bg-warn",
  agent_status: "bg-ok",
  tool_call: "bg-danger",
};

export function EventDistribution({ counts, className }: EventDistributionProps) {
  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  const sorted = Object.entries(counts).sort(([, a], [, b]) => b - a);

  return (
    <div className={cn("space-y-3", className)}>
      {/* Stacked bar */}
      {total > 0 && (
        <div className="h-3 w-full rounded-full bg-muted/40 overflow-hidden flex">
          {sorted.map(([type, count]) => (
            <div
              key={type}
              className={cn("h-full transition-all duration-500", EVENT_COLORS[type] ?? "bg-muted-foreground")}
              style={{ width: `${(count / total) * 100}%` }}
              title={`${type}: ${count}`}
            />
          ))}
        </div>
      )}

      {/* Legend */}
      <div className="grid grid-cols-2 gap-x-4 gap-y-1.5">
        {sorted.map(([type, count]) => (
          <div key={type} className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-1.5">
              <span className={cn("h-2 w-2 rounded-full shrink-0", EVENT_COLORS[type] ?? "bg-muted-foreground")} />
              <span className="text-[11px] font-mono text-muted-foreground">{type}</span>
            </div>
            <span className="text-[11px] font-mono font-medium text-foreground">{count}</span>
          </div>
        ))}
      </div>

      <div className="text-[11px] text-muted-foreground">
        {total} events total
      </div>
    </div>
  );
}
