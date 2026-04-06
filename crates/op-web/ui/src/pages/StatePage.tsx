import { useState, useMemo, memo, useCallback } from "react";
import { PageHeader, Card, Pill } from "@/components/shell/Primitives";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { useEventStore } from "@/stores/event-store";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { Search, ChevronDown, ChevronRight, Activity, Clock } from "lucide-react";
import type { EventLogEntry } from "@/types/api";

/* ── Schema inference ──────────────────────────────────── */

function inferSchema(data: unknown): any {
  if (data === null || data === undefined) return { type: "string" };
  if (typeof data === "boolean") return { type: "boolean" };
  if (typeof data === "number") return { type: "number", minimum: 0, maximum: typeof data === "number" && data > 100 ? data * 2 : 100 };
  if (typeof data === "string") return { type: "string" };
  if (Array.isArray(data)) return { type: "array", items: data.length > 0 ? inferSchema(data[0]) : { type: "string" } };
  if (typeof data === "object") {
    const props: Record<string, any> = {};
    for (const [k, v] of Object.entries(data as Record<string, unknown>)) props[k] = inferSchema(v);
    return { type: "object", properties: props };
  }
  return { type: "string" };
}

/* ── Plugin definitions (structural only — no mock values) ── */

interface PluginDef {
  id: string;
  label: string;
  icon: string;
  mutableKeys: Set<string>;
  schema?: Record<string, unknown>;
}

const PLUGIN_DEFS: PluginDef[] = [
  {
    id: "net", label: "Network", icon: "🌐",
    mutableKeys: new Set(["dns_servers", "mtu", "dhcp_enabled", "ipv6_enabled"]),
    schema: { type: "object", properties: { interface: { type: "string", title: "Interface", readOnly: true }, ip_address: { type: "string", title: "IP Address", readOnly: true }, mac_address: { type: "string", title: "MAC Address", readOnly: true }, mtu: { type: "number", title: "MTU", minimum: 68, maximum: 9000 }, dhcp_enabled: { type: "boolean", title: "DHCP Enabled" }, ipv6_enabled: { type: "boolean", title: "IPv6 Enabled" }, dns_servers: { type: "string", title: "DNS Servers" }, rx_bytes: { type: "number", title: "RX Bytes", readOnly: true }, tx_bytes: { type: "number", title: "TX Bytes", readOnly: true }, link_speed_mbps: { type: "number", title: "Link Speed (Mbps)", readOnly: true } } },
  },
  {
    id: "dinit", label: "Service Manager (dinit)", icon: "⚙️",
    mutableKeys: new Set(["restart_policy", "log_level", "watchdog_interval_s"]),
    schema: { type: "object", properties: { active_services: { type: "number", title: "Active Services", readOnly: true }, failed_services: { type: "number", title: "Failed Services", readOnly: true }, uptime_secs: { type: "number", title: "Uptime (s)", readOnly: true }, restart_policy: { type: "string", title: "Restart Policy", enum: ["always", "on-failure", "never"] }, log_level: { type: "string", title: "Log Level", enum: ["trace", "debug", "info", "warn", "error"] }, watchdog_interval_s: { type: "number", title: "Watchdog Interval (s)", minimum: 1, maximum: 300 } } },
  },
  {
    id: "wireguard", label: "WireGuard VPN", icon: "🔒",
    mutableKeys: new Set(["listen_port", "persistent_keepalive", "enabled"]),
    schema: { type: "object", properties: { enabled: { type: "boolean", title: "Enabled" }, public_key: { type: "string", title: "Public Key", readOnly: true }, endpoint: { type: "string", title: "Endpoint", readOnly: true }, listen_port: { type: "number", title: "Listen Port", minimum: 1024, maximum: 65535 }, persistent_keepalive: { type: "number", title: "Persistent Keepalive (s)", minimum: 0, maximum: 120 }, connected_peers: { type: "number", title: "Connected Peers", readOnly: true }, last_handshake: { type: "string", title: "Last Handshake", readOnly: true } } },
  },
  {
    id: "dbus", label: "D-Bus Monitor", icon: "🚌",
    mutableKeys: new Set(["trace_enabled", "max_message_size"]),
    schema: { type: "object", properties: { system_bus_active: { type: "boolean", title: "System Bus Active", readOnly: true }, session_bus_active: { type: "boolean", title: "Session Bus Active", readOnly: true }, registered_services: { type: "number", title: "Registered Services", readOnly: true }, trace_enabled: { type: "boolean", title: "Trace Enabled" }, max_message_size: { type: "number", title: "Max Message Size (bytes)", minimum: 1024, maximum: 134217728 }, messages_per_sec: { type: "number", title: "Messages/sec", readOnly: true } } },
  },
  {
    id: "system", label: "System Resources", icon: "📊",
    mutableKeys: new Set([]),
    schema: { type: "object", properties: { cpu_percent: { type: "number", title: "CPU Usage (%)", readOnly: true, minimum: 0, maximum: 100 }, memory_used_mb: { type: "number", title: "Memory Used (MB)", readOnly: true }, memory_total_mb: { type: "number", title: "Memory Total (MB)", readOnly: true }, disk_used_percent: { type: "number", title: "Disk Usage (%)", readOnly: true, minimum: 0, maximum: 100 }, load_avg_1m: { type: "number", title: "Load Avg (1m)", readOnly: true }, processes: { type: "number", title: "Processes", readOnly: true } } },
  },
];

/* ── Plugin Accordion Card ─────────────────────────────── */

const PluginCard = memo(function PluginCard({
  plugin, data, expanded, onToggle, onChange, searchMatch,
}: {
  plugin: PluginDef; data: Record<string, unknown>; expanded: boolean;
  onToggle: () => void; onChange: (updated: unknown) => void; searchMatch: boolean;
}) {
  const propCount = Object.keys(data).length;
  const mutableCount = plugin.mutableKeys.size;
  if (!searchMatch) return null;

  return (
    <div className="border border-border rounded-lg overflow-hidden">
      <button onClick={onToggle} className="w-full flex items-center gap-3 px-4 py-3 hover:bg-muted/30 transition-colors">
        <span className="text-lg">{plugin.icon}</span>
        <div className="flex-1 text-left">
          <span className="text-sm font-semibold text-foreground">{plugin.label}</span>
          <span className="text-[11px] text-muted-foreground font-mono ml-2">({plugin.id})</span>
        </div>
        <Badge variant="outline" className="text-[10px]">{propCount} props</Badge>
        {mutableCount > 0 && <Badge variant="secondary" className="text-[10px]">{mutableCount} tunable</Badge>}
        {expanded ? <ChevronDown className="h-4 w-4 text-muted-foreground" /> : <ChevronRight className="h-4 w-4 text-muted-foreground" />}
      </button>
      {expanded && (
        <div className="px-4 pb-4 border-t border-border/50">
          {propCount === 0 ? (
            <div className="text-xs text-muted-foreground py-4 text-center">No data received for this plugin yet.</div>
          ) : (
            <SchemaRenderer schema={plugin.schema ?? inferSchema(data)} data={data} onChange={onChange} className="mt-3" />
          )}
        </div>
      )}
    </div>
  );
});

/* ── Event Tape Row with Collapsible ───────────────────── */

const TapeRow = memo(function TapeRow({ event }: { event: EventLogEntry }) {
  const [open, setOpen] = useState(false);
  const time = new Date(event.ts).toLocaleTimeString();

  const eventColor = cn(
    "text-[10px] font-medium px-1.5 py-0.5 rounded-full border shrink-0",
    event.event === "state_update" && "border-info/30 text-info bg-info/10",
    event.event === "health" && "border-ok/30 text-ok bg-ok/10",
    event.event === "audit_event" && "border-warn/30 text-warn bg-warn/10",
    event.event === "system_stats" && "border-primary/30 text-primary bg-primary/10",
    !["health", "state_update", "audit_event", "system_stats"].includes(event.event) && "border-border text-muted-foreground",
  );

  const payload = event.payload as Record<string, unknown> | null;
  const stateKey = payload?.key as string | undefined;
  const newValue = payload?.new_value ?? payload;

  return (
    <div className="border-b border-border/30 last:border-0">
      <button onClick={() => setOpen(!open)} className="w-full flex items-center gap-3 px-3 py-2 hover:bg-muted/30 transition-colors text-left">
        <span className="font-mono text-[11px] text-muted-foreground w-20 shrink-0">{time}</span>
        <span className={eventColor}>{event.event}</span>
        {stateKey && <Badge variant="outline" className="text-[9px] font-mono shrink-0">{stateKey}</Badge>}
        <span className="text-xs text-muted-foreground truncate flex-1 font-mono">
          {typeof event.payload === "string" ? event.payload : JSON.stringify(event.payload)?.slice(0, 60)}
        </span>
        {open ? <ChevronDown className="h-3 w-3 text-muted-foreground shrink-0" /> : <ChevronRight className="h-3 w-3 text-muted-foreground shrink-0" />}
      </button>
      {open && newValue != null && (
        <div className="px-3 pb-3 ml-[80px]">
          {typeof newValue === "object" ? (
            <div className="border border-border/50 rounded-md p-3 bg-muted/10">
              <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-2">Changed Value</div>
              <SchemaRenderer schema={inferSchema(newValue)} data={newValue} readOnly />
            </div>
          ) : (
            <div className="border border-border/50 rounded-md p-3 bg-muted/10">
              <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-1">New Value</div>
              <Badge variant="secondary" className="font-mono text-xs">{String(newValue)}</Badge>
            </div>
          )}
        </div>
      )}
    </div>
  );
});

/* ── Main Page ─────────────────────────────────────────── */

export default function StatePage() {
  const { latestState, events } = useEventStore();
  const [search, setSearch] = useState("");
  const [expandedPlugins, setExpandedPlugins] = useState<Record<string, boolean>>({ net: true });
  const [localOverrides, setLocalOverrides] = useState<Record<string, Record<string, unknown>>>({});

  const stateUpdates = useMemo(() => events.filter((e) => e.event === "state_update"), [events]);

  // Build plugin state: live store data → local overrides (no defaults)
  const mergedPluginState = useMemo(() => {
    const merged: Record<string, Record<string, unknown>> = {};
    for (const plugin of PLUGIN_DEFS) {
      const live: Record<string, unknown> = {};
      for (const [key, value] of Object.entries(latestState)) {
        const dotParts = key.split(".");
        const colonParts = key.split(":");
        if (dotParts[0] === plugin.id && dotParts.length >= 2) {
          live[dotParts.slice(1).join(".")] = value;
        } else if (colonParts[0] === plugin.id && colonParts.length >= 2) {
          live[colonParts.slice(1).join(":")] = value;
        } else if (key === plugin.id && typeof value === "object" && value !== null) {
          Object.assign(live, value as Record<string, unknown>);
        }
      }
      merged[plugin.id] = { ...live, ...(localOverrides[plugin.id] ?? {}) };
    }
    return merged;
  }, [latestState, localOverrides]);

  const togglePlugin = useCallback((id: string) => {
    setExpandedPlugins((prev) => ({ ...prev, [id]: !prev[id] }));
  }, []);

  const handlePluginChange = useCallback((pluginId: string, updated: unknown) => {
    setLocalOverrides((prev) => ({ ...prev, [pluginId]: updated as Record<string, unknown> }));
  }, []);

  const filteredPlugins = useMemo(() => {
    if (!search) return PLUGIN_DEFS;
    const q = search.toLowerCase();
    return PLUGIN_DEFS.filter((p) => {
      if (p.label.toLowerCase().includes(q) || p.id.toLowerCase().includes(q)) return true;
      const data = mergedPluginState[p.id];
      if (data) return Object.keys(data).some((k) => k.toLowerCase().includes(q));
      return false;
    });
  }, [search, mergedPluginState]);

  const filteredEvents = useMemo(() => {
    const pool = stateUpdates.slice(-50).reverse();
    if (!search) return pool;
    const q = search.toLowerCase();
    return pool.filter((e) => {
      const payloadStr = JSON.stringify(e.payload).toLowerCase();
      return payloadStr.includes(q) || e.event.toLowerCase().includes(q);
    });
  }, [stateUpdates, search]);

  const tunableCount = PLUGIN_DEFS.reduce((s, p) => s + p.mutableKeys.size, 0);

  return (
    <>
      <PageHeader title="Live State" subtitle="Interactive state projections grouped by plugin. Tunables are editable; read-only values are displayed as badges." />

      <div className="relative max-w-md">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
        <Input placeholder="Filter plugins, keys, and events…" value={search} onChange={(e) => setSearch(e.target.value)} className="pl-8 h-8 text-xs bg-[hsl(var(--bg-elevated))]" />
      </div>

      <div className="flex gap-4 text-xs text-muted-foreground">
        <span className="flex items-center gap-1.5"><Activity className="h-3.5 w-3.5 text-primary" /><span className="text-foreground font-medium">{PLUGIN_DEFS.length}</span> plugins</span>
        <span className="flex items-center gap-1.5"><span className="text-foreground font-medium">{tunableCount}</span> tunables</span>
        <span className="flex items-center gap-1.5"><Clock className="h-3.5 w-3.5" /><span className="text-foreground font-medium">{stateUpdates.length}</span> state changes</span>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-[1fr_400px] gap-4 min-h-0">
        <div className="space-y-2">
          {filteredPlugins.map((plugin) => (
            <PluginCard key={plugin.id} plugin={plugin} data={mergedPluginState[plugin.id] ?? {}} expanded={!!expandedPlugins[plugin.id]}
              onToggle={() => togglePlugin(plugin.id)} onChange={(updated) => handlePluginChange(plugin.id, updated)} searchMatch={true} />
          ))}
          {filteredPlugins.length === 0 && <div className="text-sm text-muted-foreground text-center py-8">No plugins match "{search}"</div>}
        </div>

        <Card title="State Changes" subtitle="Click a row to expand the changed value.">
          <div className="mt-2 max-h-[calc(100vh-280px)] overflow-y-auto" style={{ scrollbarWidth: "thin" }}>
            {filteredEvents.length === 0 ? (
              <div className="text-sm text-muted-foreground py-6 text-center">{search ? `No events match "${search}"` : "No state changes received yet."}</div>
            ) : (
              filteredEvents.map((evt) => <TapeRow key={evt.id} event={evt} />)
            )}
          </div>
        </Card>
      </div>
    </>
  );
}
