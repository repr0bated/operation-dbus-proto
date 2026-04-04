import { PageHeader } from "@/components/shell/PageHeader";
import { JsonTree, JsonCodeBlock } from "@/components/json/JsonTree";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useState } from "react";

export default function InspectorPage() {
  const [input, setInput] = useState('{\n  "example": {\n    "nested": [1, 2, 3],\n    "active": true,\n    "name": "org.freedesktop.systemd1"\n  }\n}');
  const [mode, setMode] = useState<"tree" | "raw">("tree");
  const [parsed, setParsed] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);

  const handleInspect = () => {
    try {
      const data = JSON.parse(input);
      setParsed(data);
      setError(null);
    } catch (e: any) {
      setError(e.message);
      setParsed(null);
    }
  };

  return (
    <div className="flex flex-col h-full">
      <PageHeader title="Inspector" description="Deep inspection for schemas, payloads, and state objects">
        <div className="flex gap-1">
          <Button variant={mode === "tree" ? "secondary" : "ghost"} size="sm" onClick={() => setMode("tree")} className="text-xs h-6">Tree</Button>
          <Button variant={mode === "raw" ? "secondary" : "ghost"} size="sm" onClick={() => setMode("raw")} className="text-xs h-6">Raw</Button>
        </div>
      </PageHeader>
      <div className="flex-1 overflow-auto p-4">
        <div className="grid lg:grid-cols-2 gap-4 h-full">
          <div className="space-y-2">
            <h3 className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Input</h3>
            <Textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              className="font-mono text-xs min-h-[300px]"
            />
            <Button size="sm" onClick={handleInspect}>Inspect</Button>
            {error && <p className="text-xs text-destructive">{error}</p>}
          </div>
          <div className="space-y-2">
            <h3 className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Output</h3>
            {parsed ? (
              mode === "tree" ? (
                <div className="rounded border bg-card p-3 overflow-auto max-h-[500px]">
                  <JsonTree data={parsed} defaultExpanded />
                </div>
              ) : (
                <JsonCodeBlock data={parsed} />
              )
            ) : (
              <div className="rounded border bg-card p-4 text-xs text-muted-foreground text-center">
                Paste JSON and click Inspect
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
