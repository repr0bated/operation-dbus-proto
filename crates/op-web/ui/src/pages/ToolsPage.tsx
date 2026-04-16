import { useState, useMemo } from "react";
import { PageHeader, Card, Pill } from "@/components/shell/Primitives";
import { SchemaPanel } from "@/components/json/SchemaPanel";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { useEventStore } from "@/stores/event-store";
import type { Tool } from "@/types/api";

export default function ToolsPage() {
  const { latestState } = useEventStore();
  const [filter, setFilter] = useState("");
  const [selectedTool, setSelectedTool] = useState<Tool | null>(null);
  const [execResult, setExecResult] = useState<unknown>(null);
  const [execArgs, setExecArgs] = useState("{}");

  const tools = useMemo(() => {
    const raw = latestState["tools"] ?? latestState["tools.list"] ?? latestState["tools:catalog"];
    if (Array.isArray(raw)) return raw as Tool[];
    return [] as Tool[];
  }, [latestState]);

  const filtered = tools.filter((t) =>
    [t.name, t.description, t.category].join(" ").toLowerCase().includes(filter.toLowerCase())
  );

  const handleExecute = () => {
    if (!selectedTool) return;
    try {
      const parsed = JSON.parse(execArgs);
      setExecResult({ tool: selectedTool.name, input: parsed, output: { status: "ok", data: { message: "Simulated result" } }, duration: "42ms" });
    } catch { setExecResult({ error: "Invalid JSON arguments" }); }
  };

  return (
    <>
      <PageHeader title="Tools" subtitle="Searchable tool catalog with schema-first execution." />
      <Card>
        <div className="flex items-center justify-between">
          <div><div className="text-[15px] font-semibold text-foreground">Tool Catalog</div><div className="text-[13px] text-muted-foreground mt-1">Schema-driven tools exposed by the control plane.</div></div>
        </div>
        <div className="mt-4">
          <label className="space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">Filter</span>
            <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Search tools" className="w-full px-3 py-2 rounded-md border border-input bg-card text-sm focus:border-ring focus:ring-1 focus:ring-ring outline-none" />
          </label>
          <div className="text-xs text-muted-foreground mt-2">{filtered.length} shown</div>
        </div>
        <div className="mt-4 space-y-2">
          {tools.length === 0 && <div className="text-sm text-muted-foreground text-center py-8">No tools detected. Waiting for live data…</div>}
          {filtered.map((tool) => (
            <button key={tool.id} onClick={() => { setSelectedTool(tool); setExecResult(null); setExecArgs("{}"); }}
              className={`w-full text-left p-3 rounded-lg border transition-colors ${selectedTool?.id === tool.id ? "border-primary/30 bg-primary/5" : "border-border hover:border-muted-foreground/20"}`}>
              <div className="flex items-center gap-2">
                <span className="font-mono text-sm font-medium text-foreground">{tool.name}</span>
                <Pill variant={tool.enabled ? "ok" : "default"}>{tool.enabled ? "enabled" : "disabled"}</Pill>
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground">{tool.source}</span>
              </div>
              <div className="text-xs text-muted-foreground mt-1">{tool.description}</div>
            </button>
          ))}
        </div>
      </Card>

      {selectedTool && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div className="space-y-4">
            <SchemaPanel schema={selectedTool.inputSchema} />
            <Card title="Execute" subtitle="Run this tool with JSON arguments.">
              <label className="space-y-1.5 mt-2 block">
                <span className="text-xs font-medium text-muted-foreground">Arguments (JSON)</span>
                <textarea value={execArgs} onChange={(e) => setExecArgs(e.target.value)} rows={6}
                  className="w-full px-3 py-2 rounded-md border border-input bg-card text-sm font-mono focus:border-ring outline-none resize-y min-h-[120px]" />
              </label>
              <button onClick={handleExecute} className="mt-3 px-4 py-2 rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors">Execute</button>
            </Card>
          </div>
          {execResult && (
            <Card title="Result" subtitle="Execution output.">
              <JsonRenderer data={execResult} className="mt-2" />
            </Card>
          )}
        </div>
      )}
    </>
  );
}
