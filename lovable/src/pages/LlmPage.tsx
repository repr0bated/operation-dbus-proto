import { useState, useMemo } from "react";
import { PageHeader, Card, Pill, StatCard } from "@/components/shell/Primitives";
import { useEventStore } from "@/stores/event-store";

interface LlmModel {
  id: string;
  name: string;
  provider: string;
  contextWindow: number;
  active: boolean;
}

export default function LlmPage() {
  const { latestState } = useEventStore();

  const models = useMemo(() => {
    const raw = latestState["llm.models"] ?? latestState["llm:models"] ?? latestState["models"];
    if (Array.isArray(raw)) return raw as LlmModel[];
    return [] as LlmModel[];
  }, [latestState]);

  const activeModel = models.find((m) => m.active);

  return (
    <>
      <PageHeader title="LLM" subtitle="Provider status, available models, and routing." />
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <StatCard label="Active Model" value={activeModel?.id ?? "—"} variant={activeModel ? "ok" : undefined} />
        <StatCard label="Provider" value={activeModel?.provider ?? "—"} />
        <StatCard label="Context Window" value={activeModel ? `${(activeModel.contextWindow / 1000)}k` : "—"} />
      </div>
      <Card title="Models" subtitle="Available models from configured providers.">
        <div className="space-y-2 mt-3">
          {models.length === 0 && (
            <div className="text-sm text-muted-foreground text-center py-8">No models detected. Waiting for live data…</div>
          )}
          {models.map((m) => (
            <div key={m.id} className={`flex items-center justify-between p-3 rounded-lg border transition-colors ${m.active ? "border-primary/30 bg-primary/5" : "border-border"}`}>
              <div>
                <div className="text-sm font-medium text-foreground">{m.name}</div>
                <div className="text-xs text-muted-foreground font-mono">{m.provider} · {(m.contextWindow / 1000)}k context</div>
              </div>
              <div className="flex items-center gap-2">
                {m.active ? <Pill variant="ok">active</Pill> : (
                  <button className="px-3 py-1.5 rounded-md border border-border text-xs font-medium hover:bg-muted/30 transition-colors">Switch</button>
                )}
              </div>
            </div>
          ))}
        </div>
      </Card>
    </>
  );
}
