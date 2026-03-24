import { useState } from "react";
import { AppHeader } from "@/components/layout/AppHeader";
import { useStatus } from "@/hooks/useApi";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  Circle,
  Play,
  Square,
  RotateCcw,
  ChevronDown,
  ChevronRight,
  Terminal,
  GitBranch,
  Clock,
  Cpu,
} from "lucide-react";
import type { ServiceInfo } from "@/api/types";

/* ── Mock dinit services exposed via D-Bus ─────────────────── */
interface DinitService {
  name: string;
  description: string;
  state: "started" | "stopped" | "starting" | "stopping" | "error";
  type: "process" | "bgprocess" | "scripted" | "internal";
  pid?: number;
  uptime?: string;
  restarts: number;
  dbusPath: string;
  dependencies: string[];
  logSnippet?: string;
  cpuPercent?: number;
  memMB?: number;
}

const mockServices: DinitService[] = [
  {
    name: "wireguard-wg0",
    description: "WireGuard tunnel interface wg0",
    state: "started",
    type: "scripted",
    pid: 1842,
    uptime: "4d 12h 33m",
    restarts: 0,
    dbusPath: "/com/3tched/dinit/services/wireguard_wg0",
    dependencies: ["network-online", "ovs-bridge"],
    logSnippet: "[wg0] peer 5xQ…kR= handshake complete",
    cpuPercent: 0.2,
    memMB: 3.1,
  },
  {
    name: "incus.service",
    description: "Incus container manager daemon",
    state: "started",
    type: "bgprocess",
    pid: 902,
    uptime: "4d 12h 35m",
    restarts: 1,
    dbusPath: "/com/3tched/dinit/services/incus",
    dependencies: ["btrfs-mount", "network-online"],
    logSnippet: "Container ghost-node-03 status: RUNNING",
    cpuPercent: 1.8,
    memMB: 142.5,
  },
  {
    name: "ovs-vswitchd",
    description: "Open vSwitch forwarding daemon",
    state: "started",
    type: "process",
    pid: 678,
    uptime: "4d 12h 36m",
    restarts: 0,
    dbusPath: "/com/3tched/dinit/services/ovs_vswitchd",
    dependencies: ["ovsdb-server"],
    logSnippet: "bridge br-ghost: 4 ports, STP disabled",
    cpuPercent: 0.5,
    memMB: 28.7,
  },
  {
    name: "ovsdb-server",
    description: "Open vSwitch database server",
    state: "started",
    type: "process",
    pid: 654,
    uptime: "4d 12h 36m",
    restarts: 0,
    dbusPath: "/com/3tched/dinit/services/ovsdb_server",
    dependencies: ["boot-complete"],
    cpuPercent: 0.1,
    memMB: 12.4,
  },
  {
    name: "op-dbus-gateway",
    description: "op-dbus gRPC ↔ D-Bus gateway",
    state: "started",
    type: "process",
    pid: 1201,
    uptime: "4d 12h 34m",
    restarts: 0,
    dbusPath: "/com/3tched/dinit/services/op_dbus_gateway",
    dependencies: ["dbus-session", "network-online"],
    logSnippet: "gRPC listening on [::]:50051",
    cpuPercent: 0.4,
    memMB: 18.2,
  },
  {
    name: "audit-chain",
    description: "Blockchain audit trail writer",
    state: "started",
    type: "bgprocess",
    pid: 1350,
    uptime: "4d 12h 34m",
    restarts: 0,
    dbusPath: "/com/3tched/dinit/services/audit_chain",
    dependencies: ["op-dbus-gateway"],
    logSnippet: "Block #44201 committed (3 txns)",
    cpuPercent: 0.3,
    memMB: 24.8,
  },
  {
    name: "zeroclaw-indexer",
    description: "ZeroClaw code indexer service",
    state: "stopped",
    type: "scripted",
    restarts: 0,
    dbusPath: "/com/3tched/dinit/services/zeroclaw_indexer",
    dependencies: ["qdrant", "op-dbus-gateway"],
  },
  {
    name: "qdrant",
    description: "Qdrant vector database",
    state: "error",
    type: "bgprocess",
    restarts: 3,
    dbusPath: "/com/3tched/dinit/services/qdrant",
    dependencies: ["boot-complete"],
    logSnippet: "FATAL: collection 'zeroclaw_code' corrupted, recovery needed",
  },
  {
    name: "btrfs-mount",
    description: "BTRFS subvolume auto-mount",
    state: "started",
    type: "internal",
    uptime: "4d 12h 37m",
    restarts: 0,
    dbusPath: "/com/3tched/dinit/services/btrfs_mount",
    dependencies: [],
  },
  {
    name: "dbus-session",
    description: "D-Bus session bus daemon",
    state: "started",
    type: "process",
    pid: 412,
    uptime: "4d 12h 37m",
    restarts: 0,
    dbusPath: "/com/3tched/dinit/services/dbus_session",
    dependencies: ["boot-complete"],
    cpuPercent: 0.1,
    memMB: 4.2,
  },
];

const stateConfig: Record<string, { color: string; label: string }> = {
  started: { color: "bg-status-online", label: "running" },
  stopped: { color: "bg-status-unknown", label: "stopped" },
  starting: { color: "bg-status-degraded animate-pulse", label: "starting" },
  stopping: { color: "bg-status-degraded animate-pulse", label: "stopping" },
  error: { color: "bg-status-offline", label: "error" },
};

const typeColors: Record<string, string> = {
  process: "text-accent",
  bgprocess: "text-primary",
  scripted: "text-warning",
  internal: "text-muted-foreground",
};

function ServiceRow({ svc }: { svc: DinitService }) {
  const [open, setOpen] = useState(false);
  const st = stateConfig[svc.state];

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <Card className={`bg-card border-border ${svc.state === "error" ? "border-destructive/30" : ""}`}>
        <CollapsibleTrigger asChild>
          <CardContent className="p-3 flex items-center gap-3 cursor-pointer hover:bg-muted/30 transition-colors">
            {open ? (
              <ChevronDown className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
            ) : (
              <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
            )}
            <div className={`h-2.5 w-2.5 rounded-full shrink-0 ${st.color}`} />
            <div className="flex-1 min-w-0">
              <span className="text-sm font-mono font-medium text-foreground truncate block">
                {svc.name}
              </span>
            </div>
            <Badge variant="outline" className={`text-[10px] font-mono ${typeColors[svc.type]}`}>
              {svc.type}
            </Badge>
            <span className="text-[10px] font-mono text-muted-foreground w-14 text-right">
              {st.label}
            </span>
            {svc.pid && (
              <span className="text-[10px] font-mono text-muted-foreground w-16 text-right">
                PID {svc.pid}
              </span>
            )}
          </CardContent>
        </CollapsibleTrigger>

        <CollapsibleContent>
          <div className="border-t border-border px-4 py-3 space-y-3 bg-muted/20">
            {/* Description + D-Bus path */}
            <div className="space-y-1">
              <p className="text-xs text-muted-foreground">{svc.description}</p>
              <p className="text-[10px] font-mono text-accent/70">
                dbus: {svc.dbusPath}
              </p>
            </div>

            {/* Stats row */}
            <div className="flex flex-wrap gap-4 text-[11px] font-mono">
              {svc.uptime && (
                <div className="flex items-center gap-1.5 text-muted-foreground">
                  <Clock className="h-3 w-3" />
                  <span>{svc.uptime}</span>
                </div>
              )}
              {svc.cpuPercent !== undefined && (
                <div className="flex items-center gap-1.5 text-muted-foreground">
                  <Cpu className="h-3 w-3" />
                  <span>{svc.cpuPercent}% CPU</span>
                </div>
              )}
              {svc.memMB !== undefined && (
                <span className="text-muted-foreground">{svc.memMB} MB</span>
              )}
              {svc.restarts > 0 && (
                <span className="text-warning">
                  {svc.restarts} restart{svc.restarts > 1 ? "s" : ""}
                </span>
              )}
            </div>

            {/* Dependencies */}
            {svc.dependencies.length > 0 && (
              <div className="flex items-start gap-2">
                <GitBranch className="h-3 w-3 text-muted-foreground mt-0.5 shrink-0" />
                <div className="flex flex-wrap gap-1">
                  {svc.dependencies.map((dep) => (
                    <Badge key={dep} variant="secondary" className="text-[10px] font-mono">
                      {dep}
                    </Badge>
                  ))}
                </div>
              </div>
            )}

            {/* Last log line */}
            {svc.logSnippet && (
              <div className="rounded bg-muted px-2 py-1.5">
                <span className="text-[10px] font-mono text-foreground/70">{svc.logSnippet}</span>
              </div>
            )}

            {/* Controls */}
            <div className="flex gap-2 pt-1">
              {svc.state === "stopped" || svc.state === "error" ? (
                <Button size="sm" variant="outline" className="gap-1.5 text-xs font-mono h-7">
                  <Play className="h-3 w-3" /> Start
                </Button>
              ) : (
                <Button size="sm" variant="outline" className="gap-1.5 text-xs font-mono h-7">
                  <Square className="h-3 w-3" /> Stop
                </Button>
              )}
              <Button size="sm" variant="ghost" className="gap-1.5 text-xs font-mono h-7 text-muted-foreground">
                <RotateCcw className="h-3 w-3" /> Restart
              </Button>
              <Button size="sm" variant="ghost" className="gap-1.5 text-xs font-mono h-7 text-muted-foreground ml-auto">
                <Terminal className="h-3 w-3" /> Logs
              </Button>
            </div>
          </div>
        </CollapsibleContent>
      </Card>
    </Collapsible>
  );
}

export default function ServicesPage() {
  const { data: status, isLoading } = useStatus();

  // Merge live data if available, otherwise use mocks
  const liveServices: ServiceInfo[] = (status?.services as ServiceInfo[]) ?? [];
  const services = liveServices.length > 0 ? liveServices : null;

  const running = mockServices.filter((s) => s.state === "started").length;
  const errors = mockServices.filter((s) => s.state === "error").length;

  return (
    <>
      <AppHeader
        title="Services"
        subtitle={`dinit · ${mockServices.length} units · ${running} running${errors ? ` · ${errors} error` : ""}`}
      />
      <div className="flex-1 overflow-hidden flex flex-col">
        {/* Summary bar */}
        <div className="flex items-center gap-4 px-4 py-2.5 border-b border-border text-[11px] font-mono">
          <div className="flex items-center gap-1.5">
            <div className="h-2 w-2 rounded-full bg-status-online" />
            <span className="text-muted-foreground">{running} running</span>
          </div>
          <div className="flex items-center gap-1.5">
            <div className="h-2 w-2 rounded-full bg-status-unknown" />
            <span className="text-muted-foreground">
              {mockServices.filter((s) => s.state === "stopped").length} stopped
            </span>
          </div>
          {errors > 0 && (
            <div className="flex items-center gap-1.5">
              <div className="h-2 w-2 rounded-full bg-status-offline" />
              <span className="text-destructive">{errors} error</span>
            </div>
          )}
          <span className="ml-auto text-muted-foreground/60">
            via org.freedesktop.dinit1
          </span>
        </div>

        {/* Service list */}
        <ScrollArea className="flex-1">
          <div className="p-4 space-y-1.5">
            {isLoading && !services ? (
              Array.from({ length: 6 }).map((_, i) => (
                <Skeleton key={i} className="h-12 w-full rounded-lg" />
              ))
            ) : (
              mockServices.map((svc) => <ServiceRow key={svc.name} svc={svc} />)
            )}
          </div>
        </ScrollArea>
      </div>
    </>
  );
}
