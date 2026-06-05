import { useEffect, useMemo, useState } from "react";
import { PageHeader, Card, Pill, StatCard } from "@/components/shell/Primitives";
import { Switch } from "@/components/ui/switch";
import { api } from "@/lib/api";
import type { LlmModel, LlmProvider } from "@/types/api";

export default function LlmPage() {
  const [status, setStatus] = useState<LlmProvider | null>(null);
  const [providers, setProviders] = useState<string[]>([]);
  const [selectedProvider, setSelectedProvider] = useState<string>("zeroclaw");
  const [models, setModels] = useState<LlmModel[]>([]);
  const [nonSandboxed, setNonSandboxed] = useState(false);
  const [busyModel, setBusyModel] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refresh(provider = selectedProvider) {
    try {
      setError(null);
      const [nextStatus, nextProviders, nextModels] = await Promise.all([
        api.llm.status(),
        api.llm.providers(),
        api.llm.models(provider === "zeroclaw" ? undefined : provider),
      ]);
      setStatus(nextStatus);
      setProviders(["zeroclaw", ...nextProviders.providers.filter((p) => p !== "zeroclaw")]);
      setSelectedProvider(provider);
      setNonSandboxed(Boolean(nextStatus.model_non_sandboxed));
      // Ensure models is always an array of proper objects
      const modelList = Array.isArray(nextModels?.models) ? nextModels.models : [];
      setModels(modelList);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setModels([]);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function switchProvider(provider: string) {
    setSelectedProvider(provider);
    setModels([]);
    try {
      setError(null);
      if (provider !== "zeroclaw") {
        await api.llm.setProvider(provider);
      }
      await refresh(provider);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  const activeModel = useMemo(
    () => models.find((m) => m.id === status?.model || m.active === true),
    [models, status?.model],
  );

  async function switchModel(model: LlmModel) {
    setBusyModel(model.id);
    try {
      setError(null);
      await api.llm.setModel(model.id, nonSandboxed);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyModel(null);
    }
  }

  return (
    <>
      <PageHeader
        title="LLM"
        subtitle="Provider status, available models, and routing."
        actions={
          <>
            <select
              className="h-9 rounded-md border border-border bg-background px-3 text-sm text-foreground"
              value={selectedProvider}
              onChange={(event) => void switchProvider(event.target.value)}
            >
              {providers.map((provider) => (
                <option key={provider} value={provider}>{provider}</option>
              ))}
            </select>
            <label className="flex items-center gap-2 text-sm text-muted-foreground">
              <Switch checked={nonSandboxed} onCheckedChange={setNonSandboxed} />
              <span>Non-sandboxed</span>
            </label>
          </>
        }
      />
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <StatCard label="Active Model" value={activeModel?.id ?? "—"} variant={activeModel ? "ok" : undefined} />
        <StatCard label="Provider" value={selectedProvider || activeModel?.provider || status?.provider || "—"} />
        <StatCard label="Sandbox" value={nonSandboxed ? "off" : "on"} variant={nonSandboxed ? "warn" : "ok"} />
      </div>
      <Card title="Models" subtitle="Available models from configured providers.">
        <div className="space-y-2 mt-3">
          {error && (
            <div className="rounded-md border border-danger/20 bg-danger/10 px-3 py-2 text-sm text-danger">{error}</div>
          )}
          {models.length === 0 && (
            <div className="text-sm text-muted-foreground text-center py-8">No models detected. Waiting for live data…</div>
          )}
          {models.map((m) => (
            <div key={m.id} className={`flex items-center justify-between gap-3 p-3 rounded-lg border transition-colors ${m.id === activeModel?.id ? "border-primary/30 bg-primary/5" : "border-border"}`}>
              <div className="min-w-0">
                <div className="text-sm font-medium text-foreground">{m.name}</div>
                <div className="text-xs text-muted-foreground font-mono break-all">
                  {m.upstream_provider ?? m.provider ?? "unknown"} · {m.transport ?? "route"} · {m.status ?? (m.available ? "available" : "unavailable")}
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {m.id === activeModel?.id ? <Pill variant="ok">active</Pill> : (
                  <button
                    className="px-3 py-1.5 rounded-md border border-border text-xs font-medium hover:bg-muted/30 transition-colors disabled:opacity-50"
                    disabled={!m.available || busyModel === m.id}
                    onClick={() => void switchModel(m)}
                  >
                    {busyModel === m.id ? "Switching" : "Switch"}
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      </Card>
    </>
  );
}
