import { PageHeader } from "@/components/shell/PageHeader";
import { useTools, useToolExecution } from "@/hooks/useApi";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { SchemaPanel } from "@/components/json/SchemaPanel";
import { JsonTree, JsonCodeBlock } from "@/components/json/JsonTree";
import { Search, Play, ChevronRight } from "lucide-react";
import { useState } from "react";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { ToolInfo, ToolExecutionResult } from "@/api/types";

export default function ToolsPage() {
  const { data: tools, isLoading } = useTools();
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<ToolInfo | null>(null);
  const [argsText, setArgsText] = useState("{}");
  const [results, setResults] = useState<ToolExecutionResult[]>([]);
  const execution = useToolExecution();

  const filtered = tools?.filter((t) =>
    t.name.toLowerCase().includes(search.toLowerCase()) ||
    t.description?.toLowerCase().includes(search.toLowerCase()) ||
    t.category?.toLowerCase().includes(search.toLowerCase())
  );

  const handleExecute = () => {
    if (!selected) return;
    try {
      const args = JSON.parse(argsText);
      execution.mutate({ name: selected.name, args }, {
        onSuccess: (result) => setResults((prev) => [result, ...prev]),
      });
    } catch {
      // ignore parse error
    }
  };

  return (
    <div className="flex h-full">
      {/* Tool list */}
      <div className="w-72 border-r flex flex-col shrink-0">
        <div className="p-2 border-b">
          <div className="relative">
            <Search className="absolute left-2 top-2 h-3.5 w-3.5 text-muted-foreground" />
            <Input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search tools..."
              className="pl-7 h-7 text-xs"
            />
          </div>
        </div>
        <div className="flex-1 overflow-auto">
          {isLoading ? (
            <div className="p-4 text-xs text-muted-foreground">Loading tools...</div>
          ) : (
            filtered?.map((tool) => (
              <button
                key={tool.name}
                onClick={() => { setSelected(tool); setArgsText("{}"); }}
                className={cn(
                  "w-full text-left px-3 py-2 border-b border-border/50 hover:bg-accent/50 transition-colors",
                  selected?.name === tool.name && "bg-accent"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-mono text-xs font-medium truncate">{tool.name}</span>
                  <ChevronRight className="h-3 w-3 text-muted-foreground shrink-0" />
                </div>
                {tool.category && (
                  <Badge variant="outline" className="text-[9px] mt-0.5">{tool.category}</Badge>
                )}
                {tool.description && (
                  <p className="text-[10px] text-muted-foreground truncate mt-0.5">{tool.description}</p>
                )}
              </button>
            ))
          )}
          {filtered?.length === 0 && <div className="p-4 text-xs text-muted-foreground">No tools match</div>}
        </div>
        <div className="border-t px-3 py-2 text-[10px] text-muted-foreground">
          {tools?.length ?? 0} tools registered
        </div>
      </div>

      {/* Tool detail */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {selected ? (
          <>
            <PageHeader title={selected.name} description={selected.description}>
              <div className="flex gap-1">
                {selected.tags?.map((t) => <Badge key={t} variant="secondary" className="text-[9px]">{t}</Badge>)}
              </div>
            </PageHeader>
            <div className="flex-1 overflow-auto p-4 space-y-4">
              {/* Schema */}
              {selected.input_schema && Object.keys(selected.input_schema).length > 0 && (
                <div className="space-y-1">
                  <h3 className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Input Schema</h3>
                  <SchemaPanel schema={selected.input_schema} />
                </div>
              )}

              {/* Execution */}
              <div className="space-y-2">
                <h3 className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Execute</h3>
                <Textarea
                  value={argsText}
                  onChange={(e) => setArgsText(e.target.value)}
                  className="font-mono text-xs min-h-[80px]"
                  placeholder='{"key": "value"}'
                />
                <Button size="sm" onClick={handleExecute} disabled={execution.isPending}>
                  <Play className="h-3.5 w-3.5 mr-1" />
                  {execution.isPending ? "Running..." : "Execute"}
                </Button>
              </div>

              {/* Results */}
              {results.length > 0 && (
                <div className="space-y-2">
                  <h3 className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Results</h3>
                  {results.map((r, i) => (
                    <div key={i} className="rounded border p-2 space-y-1">
                      <div className="flex items-center gap-2">
                        <Badge variant={r.success ? "default" : "destructive"} className="text-[9px]">
                          {r.success ? "OK" : "ERR"}
                        </Badge>
                        <span className="text-[10px] font-mono text-muted-foreground">{r.duration_ms}ms</span>
                        <span className="text-[10px] text-muted-foreground">{new Date(r.timestamp).toLocaleTimeString()}</span>
                      </div>
                      <JsonTree data={r.result} defaultExpanded />
                      {r.error && <p className="text-xs text-destructive">{r.error}</p>}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center text-muted-foreground text-xs">
            Select a tool from the catalog
          </div>
        )}
      </div>
    </div>
  );
}
