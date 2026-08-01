import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useDashboardEventStream } from "@/hooks/useDashboardEventStream";
import type {
  AuditEventPayload,
  DashboardStreamEvent,
  StateUpdatePayload,
} from "@/lib/dashboard-stream";
import { Activity, Gauge, Radar, Rows3 } from "lucide-react";

function formatJson(value: unknown) {
  return JSON.stringify(value, null, 2);
}

function renderEventTitle(event: DashboardStreamEvent) {
  if (event.type === "state_update") {
    const payload = event.payload as StateUpdatePayload;
    return `${payload.plugin_id}.${payload.property_name}`;
  }

  if (event.type === "audit_event") {
    const payload = event.payload as AuditEventPayload;
    return `${payload.operation} -> ${payload.target}`;
  }

  return event.type;
}

export function StreamingJsonExamples() {
  const stream = useDashboardEventStream();
  const latestStates = Object.entries(stream.latestStateByKey).slice(0, 5);
  const counters = Object.entries(stream.counters).sort(([left], [right]) =>
    left.localeCompare(right),
  );

  return (
    <div className="grid grid-cols-1 xl:grid-cols-3 gap-4">
      <Card className="bg-card border-border">
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-medium flex items-center gap-2">
            <Rows3 className="h-4 w-4 text-muted-foreground" />
            Streaming JSON Tape
            <Badge variant={stream.connected ? "default" : "secondary"} className="ml-auto">
              {stream.connected ? "Live" : "Waiting"}
            </Badge>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-80 rounded-md border border-border/60 bg-muted/20 p-3">
            <div className="space-y-3">
              {stream.events.length === 0 ? (
                <p className="font-mono text-xs text-muted-foreground">
                  Waiting for named SSE events from <code>/api/events</code>.
                </p>
              ) : (
                stream.events.slice(0, 8).map((event, index) => (
                  <div key={`${event.receivedAt}-${index}`} className="space-y-1">
                    <div className="flex items-center gap-2">
                      <Badge variant="outline" className="font-mono text-[10px]">
                        {event.type}
                      </Badge>
                      <span className="text-xs text-foreground font-medium">
                        {renderEventTitle(event)}
                      </span>
                    </div>
                    <pre className="overflow-x-auto rounded bg-background/80 p-2 text-[10px] leading-4 text-muted-foreground">
                      {formatJson(event.payload)}
                    </pre>
                  </div>
                ))
              )}
            </div>
          </ScrollArea>
          <p className="mt-3 text-xs text-muted-foreground">
            Example 1: append-only rendering. Best when you need raw observability without losing
            event ordering.
          </p>
        </CardContent>
      </Card>

      <Card className="bg-card border-border">
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-medium flex items-center gap-2">
            <Gauge className="h-4 w-4 text-muted-foreground" />
            Incremental Aggregates
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            {counters.length === 0 ? (
              <p className="col-span-2 font-mono text-xs text-muted-foreground">
                No events counted yet.
              </p>
            ) : (
              counters.map(([eventType, count]) => (
                <div
                  key={eventType}
                  className="rounded-md border border-border/70 bg-muted/20 p-3"
                >
                  <p className="text-[11px] uppercase tracking-wider text-muted-foreground">
                    {eventType}
                  </p>
                  <p className="mt-1 font-mono text-lg text-foreground">{count}</p>
                </div>
              ))
            )}
          </div>
          <div className="rounded-md border border-border/70 bg-muted/20 p-3">
            <p className="text-[11px] uppercase tracking-wider text-muted-foreground">
              Latest System Stats
            </p>
            {stream.latestSystemStats ? (
              <div className="mt-2 grid grid-cols-3 gap-3 font-mono text-xs">
                <div>
                  <p className="text-muted-foreground">cpu</p>
                  <p className="text-foreground">
                    {stream.latestSystemStats.cpu_usage.toFixed(1)}%
                  </p>
                </div>
                <div>
                  <p className="text-muted-foreground">mem</p>
                  <p className="text-foreground">
                    {stream.latestSystemStats.memory_used_mb}MB
                  </p>
                </div>
                <div>
                  <p className="text-muted-foreground">uptime</p>
                  <p className="text-foreground">{stream.latestSystemStats.uptime_secs}s</p>
                </div>
              </div>
            ) : (
              <p className="mt-2 font-mono text-xs text-muted-foreground">
                Waiting for <code>system_stats</code>.
              </p>
            )}
          </div>
          <p className="text-xs text-muted-foreground">
            Example 2: reduce the stream into counters and top-line metrics. Good for cheap,
            real-time dashboard cards.
          </p>
        </CardContent>
      </Card>

      <Card className="bg-card border-border">
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-medium flex items-center gap-2">
            <Radar className="h-4 w-4 text-muted-foreground" />
            Keyed State Projection
          </CardTitle>
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-80 rounded-md border border-border/60 bg-muted/20 p-3">
            <div className="space-y-3">
              {latestStates.length === 0 ? (
                <p className="font-mono text-xs text-muted-foreground">
                  Waiting for <code>state_update</code> events to materialize a current view.
                </p>
              ) : (
                latestStates.map(([key, payload]) => (
                  <div key={key} className="rounded-md border border-border/60 bg-background/70 p-2">
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-mono text-[10px] text-muted-foreground">{key}</span>
                      <Badge variant="outline" className="text-[10px]">
                        {payload.plugin_id}
                      </Badge>
                    </div>
                    <pre className="mt-2 overflow-x-auto text-[10px] leading-4 text-foreground">
                      {formatJson(payload.new_value)}
                    </pre>
                  </div>
                ))
              )}
            </div>
          </ScrollArea>
          <p className="mt-3 text-xs text-muted-foreground">
            Example 3: overwrite-by-key rendering. Best when the dashboard should show the latest
            truth per object instead of replaying every change.
          </p>
        </CardContent>
      </Card>

      <Card className="xl:col-span-3 bg-card border-border">
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-medium flex items-center gap-2">
            <Activity className="h-4 w-4 text-muted-foreground" />
            Recommendation
          </CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground space-y-2">
          <p>
            For this repo, the strongest pattern is hybrid: keep an append-only tape for debugging,
            plus reducers that derive counters and keyed state for the main dashboard.
          </p>
          <p>
            That matches the current backend design: the server already emits typed JSON events over
            SSE, so the frontend should parse once and render multiple views from the same stream
            state instead of reparsing in each widget.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
