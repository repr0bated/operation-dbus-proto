import { useState, useMemo, useEffect } from "react";
import { PageHeader, Card, Pill, StatCard, StatusDot } from "@/components/shell/Primitives";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { Badge } from "@/components/ui/badge";
import { useEventStore } from "@/stores/event-store";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { personaService } from "@/grpc/client";
import type { PersonaDefinition } from "@/grpc/types/persona";
import { Loader2 } from "lucide-react";

interface CognitiveAgent {
  id: string;
  name: string;
  description: string;
  status: "running" | "busy" | "error" | "offline";
  capabilities: string[];
  activeSessions: number;
  memoryEntries: number;
  model: string;
  tools: string[];
  tags: string[];
  systemPrompt: string;
  configSchema: Record<string, unknown>;
  configData: Record<string, unknown>;
}

const STATUS_DOT: Record<CognitiveAgent["status"], "ok" | "warn" | "error" | "offline"> = {
  running: "ok", busy: "warn", error: "error", offline: "offline",
};

const STATUS_PILL: Record<CognitiveAgent["status"], "ok" | "warn" | "danger" | "default"> = {
  running: "ok", busy: "warn", error: "danger", offline: "default",
};

/** Merge persona definitions (schema) with live state (store) */
function mergeAgents(
  personas: PersonaDefinition[],
  storeAgents: CognitiveAgent[],
  liveState: Record<string, unknown>,
): CognitiveAgent[] {
  // Build lookup from live state
  const activeSet = new Set<string>();
  const activeData: Record<string, Partial<CognitiveAgent>> = {};

  // Check schema-sourced active agents list
  const activeList = liveState["agents.active"] ?? liveState["active_agents"];
  if (Array.isArray(activeList)) {
    for (const a of activeList) {
      const name = typeof a === "string" ? a : (a as Record<string, unknown>)?.name;
      if (name) {
        activeSet.add(String(name));
        if (typeof a === "object" && a !== null) {
          activeData[String(name)] = a as Partial<CognitiveAgent>;
        }
      }
    }
  }

  // Also check store agents
  for (const sa of storeAgents) {
    activeSet.add(sa.name);
    activeData[sa.name] = sa;
  }

  // Map personas to full agent objects
  const merged: CognitiveAgent[] = personas.map((p) => {
    const live = activeData[p.name];
    const isActive = activeSet.has(p.name);
    return {
      id: p.name,
      name: p.name,
      description: p.description ?? p.systemPrompt?.slice(0, 100) ?? "",
      status: isActive ? (live?.status ?? "running") : "offline",
      capabilities: p.tags ?? [],
      activeSessions: live?.activeSessions ?? 0,
      memoryEntries: live?.memoryEntries ?? 0,
      model: p.model,
      tools: p.tools,
      tags: p.tags,
      systemPrompt: p.systemPrompt,
      configSchema: live?.configSchema ?? {},
      configData: live?.configData ?? {},
    };
  });

  // Add store-only agents not in personas
  for (const sa of storeAgents) {
    if (!personas.some((p) => p.name === sa.name)) {
      merged.push({ ...sa, model: "", tools: [], tags: [], systemPrompt: "" });
    }
  }

  return merged;
}

export default function AgentsPage() {
  const latestState = useEventStore((s) => s.latestState);
  const storeAgents = useEventStore((s) => s.agents) as unknown as CognitiveAgent[];
  const [configAgent, setConfigAgent] = useState<CognitiveAgent | null>(null);
  const [configs, setConfigs] = useState<Record<string, Record<string, unknown>>>({});
  const [personas, setPersonas] = useState<PersonaDefinition[]>([]);
  const [personaSource, setPersonaSource] = useState<string>("");
  const [loading, setLoading] = useState(true);

  // Fetch personas from schema on mount
  useEffect(() => {
    personaService.listPersonas()
      .then((res) => {
        setPersonas(res.personas);
        setPersonaSource(res.source);
      })
      .catch(() => {
        // Fallback: try to extract from latestState
        const raw = latestState["agents"] ?? latestState["agents.list"] ?? latestState["cognitive_agents"];
        if (Array.isArray(raw)) {
          setPersonas(raw.map((a: Record<string, unknown>) => ({
            name: String(a.name ?? a.id ?? ""),
            systemPrompt: String(a.systemPrompt ?? a.description ?? ""),
            model: String(a.model ?? ""),
            tools: (a.tools as string[]) ?? [],
            tags: (a.tags as string[]) ?? (a.capabilities as string[]) ?? [],
            description: String(a.description ?? ""),
          })));
          setPersonaSource("latestState (fallback)");
        }
      })
      .finally(() => setLoading(false));
  }, [latestState]);

  const agents = useMemo(
    () => mergeAgents(personas, storeAgents, latestState),
    [personas, storeAgents, latestState],
  );

  const running = agents.filter((a) => a.status === "running").length;
  const totalSessions = agents.reduce((s, a) => s + (a.activeSessions ?? 0), 0);
  const totalMemory = agents.reduce((s, a) => s + (a.memoryEntries ?? 0), 0);

  return (
    <>
      <PageHeader title="Agents" subtitle="Available and active agents from the schema registry."
        actions={
          <div className="flex items-center gap-2">
            {personaSource && (
              <span className="text-[10px] text-muted-foreground font-mono bg-muted px-2 py-1 rounded">
                Source: {personaSource}
              </span>
            )}
            <button
              onClick={() => { setLoading(true); personaService.listPersonas().then((r) => { setPersonas(r.personas); setPersonaSource(r.source); }).catch(() => {}).finally(() => setLoading(false)); }}
              className="px-4 py-2 rounded-md border border-border bg-[hsl(var(--bg-elevated))] text-sm font-medium hover:bg-muted/30 transition-colors"
            >
              Refresh
            </button>
          </div>
        }
      />

      {loading ? (
        <div className="flex items-center justify-center h-32">
          <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
        </div>
      ) : (
        <>
          <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
            <StatCard label="Available" value={agents.length} />
            <StatCard label="Active" value={running} variant="ok" />
            <StatCard label="Offline" value={agents.length - running} variant={agents.length - running > 0 ? undefined : "ok"} />
            <StatCard label="Sessions" value={totalSessions} />
            <StatCard label="Memory" value={totalMemory.toLocaleString()} />
          </div>

          {agents.length === 0 ? (
            <div className="text-sm text-muted-foreground text-center py-12">
              No agents found in schema or live state. Ensure personas.yaml is loaded.
            </div>
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

                      {/* Model routing */}
                      {agent.model && (
                        <p className="text-[10px] font-mono text-muted-foreground mt-1.5 bg-muted/50 px-1.5 py-0.5 rounded inline-block">
                          {agent.model}
                        </p>
                      )}

                      {/* Tags / capabilities */}
                      <div className="flex flex-wrap gap-1 mt-2">
                        {(agent.tags?.length ? agent.tags : agent.capabilities ?? []).map((cap) => (
                          <Badge key={cap} variant="secondary" className="text-[10px] px-1.5 py-0">{cap}</Badge>
                        ))}
                      </div>

                      {/* Tools */}
                      {agent.tools?.length > 0 && (
                        <div className="flex flex-wrap gap-1 mt-1.5">
                          {agent.tools.slice(0, 5).map((t) => (
                            <span key={t} className="text-[9px] font-mono text-muted-foreground bg-muted px-1 py-0.5 rounded">{t}</span>
                          ))}
                          {agent.tools.length > 5 && (
                            <span className="text-[9px] text-muted-foreground">+{agent.tools.length - 5} more</span>
                          )}
                        </div>
                      )}

                      {/* Live metrics */}
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
        </>
      )}

      <Dialog open={!!configAgent} onOpenChange={(open) => !open && setConfigAgent(null)}>
        <DialogContent className="max-w-lg max-h-[80vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <StatusDot status={configAgent ? STATUS_DOT[configAgent.status] ?? "ok" : "ok"} />
              {configAgent?.name} Configuration
            </DialogTitle>
            <DialogDescription>
              {configAgent?.description}
              {configAgent?.model && (
                <span className="block text-[10px] font-mono mt-1">{configAgent.model}</span>
              )}
            </DialogDescription>
          </DialogHeader>
          {configAgent && (
            <div className="mt-2">
              {configAgent.systemPrompt && (
                <div className="mb-4">
                  <h4 className="text-xs font-semibold text-muted-foreground uppercase mb-1">System Prompt</h4>
                  <pre className="text-xs text-muted-foreground bg-muted/30 rounded p-2 whitespace-pre-wrap max-h-32 overflow-y-auto">
                    {configAgent.systemPrompt}
                  </pre>
                </div>
              )}
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
