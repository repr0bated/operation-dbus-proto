import { useState, useMemo } from "react";
import { PageHeader, Card } from "@/components/shell/Primitives";
import { Badge } from "@/components/ui/badge";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { useEventStore } from "@/stores/event-store";
import { ChevronRight, Plus } from "lucide-react";
import { cn } from "@/lib/utils";

interface FlowEntry {
  id: string;
  table: number;
  priority: number;
  match_fields: Record<string, string>;
  actions: string[];
  packet_count: number;
  byte_count: number;
}

interface FlowTable {
  id: number;
  name: string;
  flows: FlowEntry[];
}

const globalConfigSchema = {
  type: "object",
  properties: {
    controller: { type: "string", title: "Controller" },
    fail_mode: { type: "string", title: "Fail Mode", enum: ["secure", "standalone"] },
    stp_enable: { type: "boolean", title: "STP Enabled" },
    flow_eviction_threshold: { type: "number", title: "Flow Eviction Threshold" },
    protocols: { type: "string", title: "Protocols" },
  },
};

const flowEntrySchema = {
  type: "object",
  properties: {
    table: { type: "number", title: "Table ID", minimum: 0, maximum: 254 },
    priority: { type: "number", title: "Priority", minimum: 0, maximum: 65535 },
    in_port: { type: "string", title: "In Port" },
    dl_src: { type: "string", title: "Source MAC" },
    dl_dst: { type: "string", title: "Destination MAC" },
    dl_type: { type: "string", title: "EtherType", enum: ["0x0800", "0x0806", "0x86dd"] },
    nw_src: { type: "string", title: "Source IP (CIDR)" },
    nw_dst: { type: "string", title: "Destination IP (CIDR)" },
    nw_proto: { type: "number", title: "IP Protocol" },
    tp_dst: { type: "number", title: "Destination Port" },
    actions: { type: "string", title: "Actions (comma-separated)" },
  },
};

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export default function OpenFlowPage() {
  const { latestState } = useEventStore();

  const globalConfig = useMemo(() => {
    const live = latestState["openflow.config"] ?? latestState["openflow:config"];
    if (live && typeof live === "object") return live as Record<string, unknown>;
    return {};
  }, [latestState]);

  const [localConfig, setLocalConfig] = useState<Record<string, unknown>>({});
  const [addFlowOpen, setAddFlowOpen] = useState(false);
  const [newFlow, setNewFlow] = useState<Record<string, unknown>>({});
  const [openTables, setOpenTables] = useState<Record<number, boolean>>({ 0: true });

  const mergedConfig = useMemo(() => ({ ...globalConfig, ...localConfig }), [globalConfig, localConfig]);

  const tables = useMemo(() => {
    const live = latestState["openflow.tables"] ?? latestState["openflow:tables"];
    if (Array.isArray(live)) return live as FlowTable[];
    return [] as FlowTable[];
  }, [latestState]);

  return (
    <>
      <PageHeader title="OpenFlow" subtitle="Flow table explorer and rule management." />

      <Card title="Global Configuration" subtitle="OpenFlow controller and protocol settings." actions={
        mergedConfig.protocols ? <Badge variant="outline" className="text-[10px] font-mono">{String(mergedConfig.protocols)}</Badge> : null
      }>
        <div className="mt-3">
          <SchemaRenderer
            schema={globalConfigSchema as any}
            data={mergedConfig}
            onChange={(v) => setLocalConfig(v as Record<string, unknown>)}
          />
        </div>
      </Card>

      <div className="flex items-center justify-between mt-6 mb-3">
        <h3 className="text-sm font-semibold text-foreground">Flow Tables</h3>
        <button onClick={() => { setAddFlowOpen(true); setNewFlow({}); }} className="flex items-center gap-1.5 px-3 py-1.5 rounded-md border border-border text-xs font-medium hover:bg-muted/30 transition-colors">
          <Plus className="h-3 w-3" /> Add Flow
        </button>
      </div>

      <div className="space-y-3">
        {tables.length === 0 && (
          <div className="text-sm text-muted-foreground text-center py-8">No flow tables received. Waiting for live data…</div>
        )}
        {tables.map((table) => (
          <Collapsible key={table.id} open={openTables[table.id] ?? false} onOpenChange={(o) => setOpenTables((p) => ({ ...p, [table.id]: o }))}>
            <CollapsibleTrigger className="w-full flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3 hover:bg-muted/20 transition-colors">
              <div className="flex items-center gap-2">
                <ChevronRight className={cn("h-4 w-4 transition-transform text-muted-foreground", openTables[table.id] && "rotate-90")} />
                <span className="text-sm font-semibold text-foreground">Table {table.id}</span>
                <span className="text-xs text-muted-foreground">— {table.name}</span>
              </div>
              <Badge variant="outline" className="text-[10px]">{table.flows.length} flows</Badge>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <div className="ml-6 mt-2 space-y-2">
                {table.flows.map((flow) => (
                  <div key={flow.id} className="rounded-md border border-border bg-muted/10 px-4 py-3 space-y-2">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <Badge variant="secondary" className="text-[10px] font-mono">pri={flow.priority}</Badge>
                        <span className="text-xs text-muted-foreground font-mono">{formatCount(flow.packet_count)} pkts / {formatCount(flow.byte_count)} bytes</span>
                      </div>
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      <span className="text-[10px] text-muted-foreground uppercase mr-1">Match:</span>
                      {Object.keys(flow.match_fields).length === 0 ? (
                        <Badge variant="outline" className="text-[10px]">any</Badge>
                      ) : Object.entries(flow.match_fields).map(([k, v]) => (
                        <Badge key={k} variant="outline" className="text-[10px] font-mono">{k}={v}</Badge>
                      ))}
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      <span className="text-[10px] text-muted-foreground uppercase mr-1">Actions:</span>
                      {flow.actions.map((a, i) => (
                        <Badge key={i} className="text-[10px] font-mono bg-primary/15 text-primary border-primary/20">{a}</Badge>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </CollapsibleContent>
          </Collapsible>
        ))}
      </div>

      <Dialog open={addFlowOpen} onOpenChange={setAddFlowOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add Flow Entry</DialogTitle>
            <DialogDescription>Define match fields and actions for a new OpenFlow rule.</DialogDescription>
          </DialogHeader>
          <div className="mt-4">
            <SchemaRenderer schema={flowEntrySchema as any} data={newFlow} onChange={(v) => setNewFlow(v as Record<string, unknown>)} />
          </div>
          <div className="flex justify-end mt-4">
            <button className="px-4 py-2 rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors">Add Flow</button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
