import { useState, useMemo } from "react";
import { PageHeader, Card, Pill, StatCard, StatusDot } from "@/components/shell/Primitives";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { Badge } from "@/components/ui/badge";
import { useEventStore } from "@/stores/event-store";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";

interface CognitiveAgent {
  id: string;
  name: string;
  description: string;
  status: "running" | "busy" | "error" | "offline";
  capabilities: string[];
  activeSessions: number;
  memoryEntries: number;
  configSchema: Record<string, unknown>;
  configData: Record<string, unknown>;
}

const STATUS_DOT: Record<CognitiveAgent["status"], "ok" | "warn" | "error" | "offline"> = {
  running: "ok", busy: "warn", error: "error", offline: "offline",
};

const STATUS_PILL: Record<CognitiveAgent["status"], "ok" | "warn" | "danger" | "default"> = {
  running: "ok", busy: "warn", error: "danger", offline: "default",
};

export default function AgentsPage() {
  const latestState = useEventStore((s) => s.latestState);
  const [configAgent, setConfigAgent] = useState<CognitiveAgent | null>(null);
  const [configs, setConfigs] = useState<Record<string, Record<string, unknown>>>({});

  const agents = useMemo(() => {
    const raw = latestState["agents"] ?? latestState["agents.list"] ?? latestState["cognitive_agents"];
    if (Array.isArray(raw)) return raw as CognitiveAgent[];
    return [] as CognitiveAgent[];
  }, [latestState]);

  const running = agents.filter((a) => a.status === "running").length;
  const totalSessions = agents.reduce((s, a) => s + (a.activeSessions ?? 0), 0);
  const totalMemory = agents.reduce((s, a) => s + (a.memoryEntries ?? 0), 0);

  return (
    <>
      <PageHeader title="Cognitive Agents" subtitle="Monitor and configure always-on MCP background agents."
        actions={<button className="px-4 py-2 rounded-md border border-border bg-[hsl(var(--bg-elevated))] text-sm font-medium hover:bg-muted/30 transition-colors">Refresh</button>}
      />

      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <StatCard label="Total Agents" value={agents.length} />
        <StatCard label="Running" value={running} variant="ok" />
        <StatCard label="Active Sessions" value={totalSessions} />
        <StatCard label="Memory Entries" value={totalMemory.toLocaleString()} />
      </div>

      {agents.length === 0 ? (
        <div className="text-sm text-muted-foreground text-center py-12">No cognitive agents detected. Waiting for live data…</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {agents.map((agent) => (
            <Card key={agent.id}>
              <div className="flex items-start gap-3">
                <StatusDot status={STATUS_DOT[agent.status] ?? "offline"} className="mt-1.5" />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center justify-between gap-2">
                    <h3 className="text-sm font-bold text-foreground truncate">{agent.name}</h3>
                    <Pill variant={STATUS_PILL[agent.status] ?? "default"}>{agent.status}</Pill>
                  </div>
                  <p className="text-xs text-muted-foreground mt-1 line-clamp-2">{agent.description}</p>
                  <div className="flex flex-wrap gap-1 mt-2">
                    {(agent.capabilities ?? []).map((cap) => (
                      <Badge key={cap} variant="secondary" className="text-[10px] px-1.5 py-0">{cap}</Badge>
                    ))}
                  </div>
                  <div className="flex gap-4 mt-3 text-xs text-muted-foreground font-mono">
                    <span>Sessions: <span className="text-foreground">{agent.activeSessions ?? 0}</span></span>
                    <span>Memory: <span className="text-foreground">{(agent.memoryEntries ?? 0).toLocaleString()}</span></span>
                  </div>
                  <button onClick={() => setConfigAgent(agent)}
                    className="mt-3 w-full px-3 py-1.5 rounded-md border border-border text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-muted/30 transition-colors">
                    Configure
                  </button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!configAgent} onOpenChange={(open) => !open && setConfigAgent(null)}>
        <DialogContent className="max-w-lg max-h-[80vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <StatusDot status={configAgent ? STATUS_DOT[configAgent.status] ?? "ok" : "ok"} />
              {configAgent?.name} Configuration
            </DialogTitle>
            <DialogDescription>{configAgent?.description}</DialogDescription>
          </DialogHeader>
          {configAgent && (
            <div className="mt-2">
              <SchemaRenderer
                schema={configAgent.configSchema ?? {}}
                data={configs[configAgent.id] ?? configAgent.configData ?? {}}
                onChange={(updated) => setConfigs((prev) => ({ ...prev, [configAgent.id]: updated as Record<string, unknown> }))}
              />
              <div className="flex justify-end gap-2 mt-4">
                <button onClick={() => setConfigAgent(null)} className="px-4 py-2 rounded-md border border-border text-sm font-medium text-muted-foreground hover:text-foreground transition-colors">Cancel</button>
                <button onClick={() => setConfigAgent(null)} className="px-4 py-2 rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors">Save</button>
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}
