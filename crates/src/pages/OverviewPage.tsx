import { AppHeader } from "@/components/layout/AppHeader";
import { StreamingJsonExamples } from "@/components/dashboard/StreamingJsonExamples";
import { useHealth, useStatus } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Activity, Bot, Clock, Cpu, Heart, Wrench } from "lucide-react";
import type { ComponentStatus } from "@/api/types";

function StatusDot({ status }: { status: ComponentStatus }) {
  const color: Record<ComponentStatus, string> = {
    healthy: "bg-status-online",
    degraded: "bg-status-degraded",
    unhealthy: "bg-status-offline",
    unknown: "bg-status-unknown",
  };
  return (
    <span
      className={`inline-block h-2 w-2 rounded-full ${color[status] ?? color.unknown} ${status === "healthy" ? "animate-pulse-dot" : ""}`}
    />
  );
}

function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export default function OverviewPage() {
  const { data: health, isLoading: healthLoading, isError: healthError } = useHealth();
  const { data: status, isLoading: statusLoading } = useStatus();

  return (
    <>
      <AppHeader title="Overview" subtitle="system status" />
      <div className="flex-1 overflow-auto p-4 md:p-6 space-y-6">
        {/* Metric cards */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <Card className="bg-card border-border">
            <CardContent className="p-4 flex items-center gap-3">
              <div className="rounded-md bg-primary/10 p-2">
                <Heart className="h-4 w-4 text-primary" />
              </div>
              <div>
                <p className="text-[11px] text-muted-foreground uppercase tracking-wider">Health</p>
                {healthLoading ? (
                  <Skeleton className="h-5 w-16 mt-1" />
                ) : healthError ? (
                  <Badge variant="destructive" className="text-xs mt-1">Offline</Badge>
                ) : (
                  <Badge
                    variant={health?.healthy ? "default" : "destructive"}
                    className="text-xs mt-1"
                  >
                    {health?.healthy ? "Online" : "Degraded"}
                  </Badge>
                )}
              </div>
            </CardContent>
          </Card>

          <Card className="bg-card border-border">
            <CardContent className="p-4 flex items-center gap-3">
              <div className="rounded-md bg-info/10 p-2">
                <Clock className="h-4 w-4 text-info" />
              </div>
              <div>
                <p className="text-[11px] text-muted-foreground uppercase tracking-wider">Uptime</p>
                {healthLoading ? (
                  <Skeleton className="h-5 w-20 mt-1" />
                ) : (
                  <p className="text-sm font-mono font-medium text-foreground mt-1">
                    {health ? formatUptime(health.uptime_secs) : "—"}
                  </p>
                )}
              </div>
            </CardContent>
          </Card>

          <Card className="bg-card border-border">
            <CardContent className="p-4 flex items-center gap-3">
              <div className="rounded-md bg-accent/10 p-2">
                <Wrench className="h-4 w-4 text-accent" />
              </div>
              <div>
                <p className="text-[11px] text-muted-foreground uppercase tracking-wider">Tools</p>
                {statusLoading ? (
                  <Skeleton className="h-5 w-12 mt-1" />
                ) : (
                  <p className="text-sm font-mono font-medium text-foreground mt-1">
                    {status?.tools_count ?? "—"}
                  </p>
                )}
              </div>
            </CardContent>
          </Card>

          <Card className="bg-card border-border">
            <CardContent className="p-4 flex items-center gap-3">
              <div className="rounded-md bg-warning/10 p-2">
                <Bot className="h-4 w-4 text-warning" />
              </div>
              <div>
                <p className="text-[11px] text-muted-foreground uppercase tracking-wider">Agents</p>
                {statusLoading ? (
                  <Skeleton className="h-5 w-12 mt-1" />
                ) : (
                  <p className="text-sm font-mono font-medium text-foreground mt-1">
                    {status?.agents_count ?? "—"}
                  </p>
                )}
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Version + Component Health */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <Card className="bg-card border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium flex items-center gap-2">
                <Cpu className="h-4 w-4 text-muted-foreground" />
                System Info
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              {healthLoading ? (
                <div className="space-y-2">
                  <Skeleton className="h-4 w-full" />
                  <Skeleton className="h-4 w-3/4" />
                </div>
              ) : (
                <div className="font-mono text-xs space-y-1">
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">version</span>
                    <span className="text-foreground">{health?.version ?? "—"}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">uptime</span>
                    <span className="text-foreground">{health ? `${health.uptime_secs}s` : "—"}</span>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>

          <Card className="bg-card border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium flex items-center gap-2">
                <Activity className="h-4 w-4 text-muted-foreground" />
                Components
              </CardTitle>
            </CardHeader>
            <CardContent>
              {healthLoading ? (
                <div className="space-y-2">
                  <Skeleton className="h-4 w-full" />
                  <Skeleton className="h-4 w-full" />
                  <Skeleton className="h-4 w-full" />
                </div>
              ) : health?.components ? (
                <div className="space-y-1.5">
                  {Object.entries(health.components).map(([key, comp]) => (
                    <div key={key} className="flex items-center justify-between font-mono text-xs">
                      <div className="flex items-center gap-2">
                        <StatusDot status={comp.status} />
                        <span className="text-foreground">{comp.name || key}</span>
                      </div>
                      <span className="text-muted-foreground">{comp.status}</span>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-xs text-muted-foreground font-mono">
                  No component data available
                </p>
              )}
            </CardContent>
          </Card>
        </div>

        <div className="space-y-3">
          <div>
            <h2 className="text-sm font-semibold text-foreground">Streaming JSON Rendering</h2>
            <p className="text-sm text-muted-foreground">
              Examples using the live SSE event bus to render raw events, derived metrics, and
              keyed state projections.
            </p>
          </div>
          <StreamingJsonExamples />
        </div>
      </div>
    </>
  );
}
