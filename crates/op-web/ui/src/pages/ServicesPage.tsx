import { useState, useMemo } from "react";
import { PageHeader, Card, Pill, StatusDot } from "@/components/shell/Primitives";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { useEventStore } from "@/stores/event-store";
import { cn } from "@/lib/utils";
import {
  ChevronRight, Play, Square, RotateCcw, FileText,
  Cpu, MemoryStick, Clock, Activity,
} from "lucide-react";

/* ── Types ─────────────────────────────────────────────── */

type DinitState = "started" | "stopped" | "starting" | "stopping" | "error";

interface DinitService {
  name: string;
  bus: string;
  pid?: number;
  uniqueName?: string;
  unique_name?: string;
  interfaces?: string[];
  state?: DinitState;
  uptime?: string;
  uptime_secs?: number;
  restarts?: number;
  cpu?: number;
  memory_mb?: number;
  is_activatable?: boolean;
  cmdline?: string;
  metadata?: Record<string, unknown>;
  configSchema?: Record<string, unknown>;
  configData?: Record<string, unknown>;
  actions?: string[];
}

/* ── Helpers ───────────────────────────────────────────── */

const STATE_BADGE: Record<DinitState, { variant: "ok" | "warn" | "danger" | "default"; dot: "ok" | "warn" | "error" | "offline" }> = {
  started: { variant: "ok", dot: "ok" },
  starting: { variant: "warn", dot: "warn" },
  stopping: { variant: "warn", dot: "warn" },
  stopped: { variant: "default", dot: "offline" },
  error: { variant: "danger", dot: "error" },
};

function formatUptime(svc: DinitService): string {
  if (svc.uptime) return svc.uptime;
  if (svc.uptime_secs != null) {
    const s = svc.uptime_secs;
    if (s < 60) return `${s}s`;
    if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
    return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
  }
  return "—";
}

/* ── Component ─────────────────────────────────────────── */

export default function ServicesPage() {
  const { latestState } = useEventStore();
  const [expandedRows, setExpandedRows] = useState<Set<string>>(new Set());
  const [localConfigs, setLocalConfigs] = useState<Record<string, Record<string, unknown>>>({});

  const services = useMemo(() => {
    const raw = latestState["services"] ?? latestState["services.list"] ?? latestState["dinit:services"];
    if (Array.isArray(raw)) return raw as DinitService[];
    return [] as DinitService[];
  }, [latestState]);

  const toggleRow = (name: string) => {
    setExpandedRows((prev) => {
      const next = new Set(prev);
      next.has(name) ? next.delete(name) : next.add(name);
      return next;
    });
  };

  const started = services.filter((s) => s.state === "started").length;
  const errored = services.filter((s) => s.state === "error").length;

  return (
    <>
      <PageHeader
        title="Dinit Services"
        subtitle="Service manager control center — monitor, inspect, and manage dinit services."
        actions={
          <button className="px-4 py-2 rounded-md border border-border bg-[hsl(var(--bg-elevated))] text-sm font-medium hover:bg-muted/30 transition-colors">
            Refresh
          </button>
        }
      />

      {/* KPI row */}
      <div className="flex gap-4 text-xs text-muted-foreground mb-4">
        <span className="flex items-center gap-1.5">
          <Activity className="h-3.5 w-3.5 text-primary" />
          <span className="text-foreground font-medium">{services.length}</span> services
        </span>
        {services.length > 0 && (
          <>
            <span className="flex items-center gap-1.5">
              <span className="text-ok font-medium">{started}</span> started
            </span>
            {errored > 0 && (
              <span className="flex items-center gap-1.5">
                <span className="text-danger font-medium">{errored}</span> errors
              </span>
            )}
          </>
        )}
      </div>

      <Card>
        {/* Table header */}
        <div className="hidden md:flex items-center gap-3 px-3 py-2 text-[11px] font-medium text-muted-foreground uppercase tracking-wider border-b border-border/50">
          <div className="w-5" />
          <div className="flex-1">Service</div>
          <div className="w-20 text-center">State</div>
          <div className="w-16 text-center">Bus</div>
          <div className="w-20 text-right">Uptime</div>
          <div className="w-16 text-right">Restarts</div>
          <div className="w-16 text-right">CPU</div>
          <div className="w-20 text-right">Memory</div>
        </div>

        <div className="space-y-0">
          {services.length === 0 && (
            <div className="text-sm text-muted-foreground text-center py-12">
              No dinit services detected. Waiting for live data…
            </div>
          )}
          {services.map((svc) => {
            const isExpanded = expandedRows.has(svc.name);
            const state = svc.state ?? "stopped";
            const badge = STATE_BADGE[state] ?? STATE_BADGE.stopped;
            const uname = svc.uniqueName ?? svc.unique_name;

            return (
              <Collapsible key={svc.name} open={isExpanded} onOpenChange={() => toggleRow(svc.name)}>
                <CollapsibleTrigger asChild>
                  <button className="w-full flex items-center gap-3 px-3 py-3 hover:bg-muted/10 transition-colors border-b border-border/30 last:border-0 text-left">
                    <ChevronRight className={cn("h-3.5 w-3.5 text-muted-foreground transition-transform shrink-0", isExpanded && "rotate-90")} />

                    {/* Service name */}
                    <div className="flex-1 min-w-0">
                      <div className="font-mono text-sm font-medium text-foreground truncate">{svc.name}</div>
                      <div className="text-[11px] text-muted-foreground mt-0.5 truncate md:hidden">
                        {svc.pid != null && `PID ${svc.pid}`} {uname && `· ${uname}`} {svc.interfaces && `· ${svc.interfaces.length} ifaces`}
                      </div>
                      <div className="hidden md:block text-[11px] text-muted-foreground mt-0.5 truncate">
                        {svc.pid != null && `PID ${svc.pid}`} {uname && `· ${uname}`} {svc.interfaces && `· ${svc.interfaces.length} interfaces`}
                      </div>
                    </div>

                    {/* State */}
                    <div className="w-20 flex justify-center">
                      <div className="flex items-center gap-1.5">
                        <StatusDot status={badge.dot} />
                        <Pill variant={badge.variant}>{state}</Pill>
                      </div>
                    </div>

                    {/* Bus */}
                    <div className="w-16 hidden md:flex justify-center">
                      <Badge variant="outline" className="text-[10px] font-mono">{svc.bus}</Badge>
                    </div>

                    {/* Uptime */}
                    <div className="w-20 hidden md:flex justify-end">
                      <span className="text-xs text-muted-foreground font-mono flex items-center gap-1">
                        <Clock className="h-3 w-3" />
                        {formatUptime(svc)}
                      </span>
                    </div>

                    {/* Restarts */}
                    <div className="w-16 hidden md:flex justify-end">
                      <span className={cn("text-xs font-mono", (svc.restarts ?? 0) > 0 ? "text-warn" : "text-muted-foreground")}>
                        {svc.restarts ?? 0}
                      </span>
                    </div>

                    {/* CPU */}
                    <div className="w-16 hidden md:flex justify-end">
                      {svc.cpu != null ? (
                        <span className="text-xs font-mono text-muted-foreground flex items-center gap-1">
                          <Cpu className="h-3 w-3" />{svc.cpu.toFixed(1)}%
                        </span>
                      ) : (
                        <span className="text-xs text-muted-foreground">—</span>
                      )}
                    </div>

                    {/* Memory */}
                    <div className="w-20 hidden md:flex justify-end">
                      {svc.memory_mb != null ? (
                        <span className="text-xs font-mono text-muted-foreground flex items-center gap-1">
                          <MemoryStick className="h-3 w-3" />{svc.memory_mb.toFixed(0)} MB
                        </span>
                      ) : (
                        <span className="text-xs text-muted-foreground">—</span>
                      )}
                    </div>
                  </button>
                </CollapsibleTrigger>

                <CollapsibleContent>
                  <div className="px-4 py-4 ml-6 border-l-2 border-primary/20 space-y-4 bg-muted/5">
                    {/* Action bar */}
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mr-1">Actions:</span>
                      {(svc.actions ?? ["Restart", "Stop", "View Logs"]).map((action) => {
                        const isDestructive = action.toLowerCase() === "stop";
                        const icon = action.toLowerCase().includes("restart") ? RotateCcw
                          : action.toLowerCase().includes("stop") ? Square
                          : action.toLowerCase().includes("log") ? FileText
                          : Play;
                        const Icon = icon;
                        return (
                          <Button
                            key={action}
                            size="sm"
                            variant={isDestructive ? "destructive" : "outline"}
                            className="h-7 text-xs gap-1.5"
                            onClick={(e) => { e.stopPropagation(); console.log(`[dinit] ${action} → ${svc.name}`); }}
                          >
                            <Icon className="h-3 w-3" />
                            {action}
                          </Button>
                        );
                      })}
                    </div>

                    {/* Metadata / D-Bus schema via JsonRenderer */}
                    {svc.metadata && Object.keys(svc.metadata).length > 0 && (
                      <div>
                        <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-2">Service Metadata</div>
                        <JsonRenderer data={svc.metadata} defaultMode="tree" />
                      </div>
                    )}

                    {/* Tunable config via SchemaRenderer */}
                    {svc.configSchema && (
                      <div>
                        <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-2">Configuration</div>
                        <SchemaRenderer
                          schema={svc.configSchema}
                          data={localConfigs[svc.name] ?? svc.configData ?? {}}
                          onChange={(val) => setLocalConfigs((p) => ({
                            ...p,
                            [svc.name]: val as Record<string, unknown>,
                          }))}
                        />
                      </div>
                    )}

                    {/* Fallback: show all service data if no specific metadata/config */}
                    {!svc.metadata && !svc.configSchema && (
                      <div>
                        <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-2">Service Details</div>
                        <JsonRenderer
                          data={{
                            name: svc.name,
                            bus: svc.bus,
                            pid: svc.pid,
                            unique_name: uname,
                            interfaces: svc.interfaces,
                            is_activatable: svc.is_activatable,
                            cmdline: svc.cmdline,
                          }}
                          defaultMode="tree"
                        />
                      </div>
                    )}
                  </div>
                </CollapsibleContent>
              </Collapsible>
            );
          })}
        </div>
      </Card>
    </>
  );
}
