import { useState } from "react";
import { PageHeader, Card, Pill, StatCard } from "@/components/shell/Primitives";
import { JsonRenderer } from "@/components/json/JsonRenderer";

const MOCK_AGENTS = [
  { id: "main", name: "Main Agent", status: "running" as const, model: "gpt-4o", tools: ["dbus.list_services", "dbus.introspect", "dbus.call_method"], sessionKey: "agent:main", lastActive: new Date().toISOString(), identity: { name: "DBUS Agent", emoji: "🤖" } },
  { id: "monitor", name: "Monitor Agent", status: "idle" as const, model: "gpt-4o-mini", tools: ["system.exec"], sessionKey: "agent:monitor", lastActive: null, identity: { name: "Monitor", emoji: "👁️" } },
  { id: "security", name: "Security Scanner", status: "stopped" as const, model: "gpt-4o", tools: ["dbus.introspect"], sessionKey: "agent:security", lastActive: null, identity: { name: "SecBot", emoji: "🛡️" } },
];

type Panel = "overview" | "tools" | "files";

export default function AgentsPage() {
  const [selectedId, setSelectedId] = useState<string>("main");
  const [panel, setPanel] = useState<Panel>("overview");
  const agent = MOCK_AGENTS.find((a) => a.id === selectedId);

  return (
    <>
      <PageHeader title="Routable Models" subtitle="LLM gateway model routing and configuration." actions={
        <button className="px-4 py-2 rounded-md border border-border bg-[hsl(var(--bg-elevated))] text-sm font-medium hover:bg-muted/30 transition-colors">Refresh</button>
      } />
      <div className="grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-4">
        {/* Agent list */}
        <Card title="Agents">
          <div className="space-y-1 mt-2">
            {MOCK_AGENTS.map((a) => (
              <button key={a.id} onClick={() => setSelectedId(a.id)}
                className={`w-full text-left p-3 rounded-lg border transition-colors ${selectedId === a.id ? "border-primary/30 bg-primary/5" : "border-transparent hover:bg-muted/30"}`}>
                <div className="flex items-center gap-2">
                  <span className="text-lg">{a.identity?.emoji}</span>
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-medium text-foreground truncate">{a.name}</div>
                    <div className="text-[11px] text-muted-foreground font-mono">{a.model}</div>
                  </div>
                  <Pill variant={a.status === "running" ? "ok" : (a.status as string) === "error" ? "danger" : "default"}>{a.status}</Pill>
                </div>
              </button>
            ))}
          </div>
        </Card>

        {/* Agent detail */}
        {agent && (
          <div className="space-y-4">
            <Card>
              <div className="flex items-center gap-3 mb-4">
                <span className="text-2xl">{agent.identity?.emoji}</span>
                <div>
                  <div className="text-lg font-bold text-foreground">{agent.name}</div>
                  <div className="text-xs text-muted-foreground font-mono">{agent.sessionKey}</div>
                </div>
                <div className="ml-auto flex gap-2">
                  {(["overview", "tools", "files"] as Panel[]).map((p) => (
                    <button key={p} onClick={() => setPanel(p)}
                      className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${panel === p ? "bg-primary/15 text-primary" : "text-muted-foreground hover:text-foreground"}`}>
                      {p.charAt(0).toUpperCase() + p.slice(1)}
                    </button>
                  ))}
                </div>
              </div>
              <div className="grid grid-cols-3 gap-3">
                <StatCard label="Status" value={agent.status} variant={agent.status === "running" ? "ok" : "default"} />
                <StatCard label="Model" value={agent.model} />
                <StatCard label="Tools" value={agent.tools.length} />
              </div>
            </Card>
            {panel === "tools" && (
              <Card title="Tool Access">
                <div className="space-y-1 mt-2">
                  {agent.tools.map((t) => (
                    <div key={t} className="flex items-center justify-between px-3 py-2 rounded-md border border-border">
                      <span className="font-mono text-sm">{t}</span>
                      <Pill variant="ok">allowed</Pill>
                    </div>
                  ))}
                </div>
              </Card>
            )}
            {panel === "overview" && (
              <Card title="Identity">
                <JsonRenderer data={agent} className="mt-2" />
              </Card>
            )}
          </div>
        )}
      </div>
    </>
  );
}
