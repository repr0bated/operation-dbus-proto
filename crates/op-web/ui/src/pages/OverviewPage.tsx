import { PageHeader } from "@/components/shell/PageHeader";
import { MetricCard } from "@/components/shell/MetricCard";
import { StatusBadge } from "@/components/shell/StatusBadge";
import { EventTape } from "@/components/json/EventTape";
import { StateProjectionPanel } from "@/components/json/StateProjectionPanel";
import { useEventStream } from "@/api/eventStream";
import { useHealth, useStatus } from "@/hooks/useApi";
import { Server, Wrench, Bot, Brain, Activity, AlertTriangle } from "lucide-react";
import { useEffect } from "react";

export default function OverviewPage() {
  const { data: health } = useHealth();
  const { data: status } = useStatus();
  const { connect, disconnect, events, counters, connected, latestStats } = useEventStream();

  useEffect(() => {
    connect();
    return () => disconnect();
  }, [connect, disconnect]);

  return (
    <div className="flex flex-col h-full">
      <PageHeader title="Overview" description="Control plane status and live telemetry" />
      <div className="flex-1 overflow-auto p-4 space-y-4">
        {/* Health bar */}
        <div className="rounded border bg-card p-3 flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="text-xs font-medium">Gateway</span>
            <StatusBadge status={health?.status || "disconnected"} pulse />
          </div>
          {health && (
            <>
              <span className="text-[10px] text-muted-foreground font-mono">v{health.version}</span>
              <span className="text-[10px] text-muted-foreground font-mono">uptime {Math.floor((health.uptime_seconds || 0) / 60)}m</span>
              {Object.entries(health.components || {}).map(([k, v]) => (
                <div key={k} className="flex items-center gap-1">
                  <StatusBadge status={v.status} label={k} />
                  {v.latency_ms !== undefined && <span className="text-[10px] text-muted-foreground font-mono">{v.latency_ms}ms</span>}
                </div>
              ))}
            </>
          )}
          {!health && <span className="text-[10px] text-muted-foreground">Waiting for health data...</span>}
        </div>

        {/* KPI row */}
        <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-3">
          <MetricCard label="Nodes" value={status?.node_count ?? "—"} icon={<Server className="h-3.5 w-3.5" />} />
          <MetricCard label="Services" value={status?.service_count ?? "—"} icon={<Activity className="h-3.5 w-3.5" />} />
          <MetricCard label="Tools" value={status?.tool_count ?? "—"} icon={<Wrench className="h-3.5 w-3.5" />} />
          <MetricCard label="Agents" value={status?.agent_count ?? "—"} icon={<Bot className="h-3.5 w-3.5" />} />
          <MetricCard label="Sessions" value={status?.active_sessions ?? "—"} icon={<AlertTriangle className="h-3.5 w-3.5" />} />
          <MetricCard
            label="LLM"
            value={status?.llm_status?.model || "—"}
            sub={status?.llm_status?.provider}
            icon={<Brain className="h-3.5 w-3.5" />}
          />
        </div>

        {/* Bus connections */}
        {status?.bus_connections && (
          <div className="flex gap-2">
            <StatusBadge status={status.bus_connections.system ? "connected" : "disconnected"} label="System Bus" />
            <StatusBadge status={status.bus_connections.session ? "connected" : "disconnected"} label="Session Bus" />
          </div>
        )}

        {/* Two-column: events + state */}
        <div className="grid lg:grid-cols-2 gap-4">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Live Events</h2>
              <div className="flex gap-2">
                {Object.entries(counters).map(([k, v]) => (
                  <span key={k} className="text-[10px] font-mono text-muted-foreground">{k}:{v}</span>
                ))}
              </div>
            </div>
            <EventTape events={events.slice(0, 50)} maxHeight="360px" />
          </div>

          <div className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Projected State</h2>
            <StateProjectionPanel />
            {latestStats && (
              <div className="rounded border bg-card p-2">
                <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground mb-1">System Stats</div>
                <div className="font-mono text-xs text-muted-foreground">
                  {Object.entries(latestStats).slice(0, 6).map(([k, v]) => (
                    <div key={k} className="flex justify-between py-px">
                      <span>{k}</span>
                      <span className="text-foreground">{JSON.stringify(v)}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
