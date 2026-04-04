import { PageHeader } from "@/components/shell/PageHeader";
import { JsonTree, JsonCodeBlock } from "@/components/json/JsonTree";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useState } from "react";
import { cn } from "@/lib/utils";

const sections = [
  { key: "general", label: "General" },
  { key: "dbus", label: "D-Bus" },
  { key: "llm", label: "LLM" },
  { key: "agents", label: "Agents" },
  { key: "security", label: "Security" },
  { key: "logging", label: "Logging" },
  { key: "network", label: "Network" },
];

export default function ConfigPage() {
  const [activeSection, setActiveSection] = useState("general");
  const [mode, setMode] = useState<"structured" | "raw">("structured");
  const [rawJson, setRawJson] = useState("{}");

  return (
    <div className="flex h-full">
      {/* Section nav */}
      <div className="w-44 border-r shrink-0 py-2">
        {sections.map((s) => (
          <button
            key={s.key}
            onClick={() => setActiveSection(s.key)}
            className={cn(
              "w-full text-left px-3 py-1.5 text-xs transition-colors",
              activeSection === s.key ? "bg-accent text-foreground font-medium" : "text-muted-foreground hover:bg-accent/50"
            )}
          >
            {s.label}
          </button>
        ))}
      </div>

      {/* Editor */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <PageHeader title={`Config: ${activeSection}`} description="Edit configuration values">
          <div className="flex gap-1">
            <Button variant={mode === "structured" ? "secondary" : "ghost"} size="sm" onClick={() => setMode("structured")} className="text-xs h-6">
              Structured
            </Button>
            <Button variant={mode === "raw" ? "secondary" : "ghost"} size="sm" onClick={() => setMode("raw")} className="text-xs h-6">
              Raw JSON
            </Button>
          </div>
        </PageHeader>
        <div className="flex-1 overflow-auto p-4">
          {mode === "structured" ? (
            <div className="rounded border bg-card p-4 text-xs text-muted-foreground text-center">
              Connect to the backend to load configuration for <span className="font-mono font-medium text-foreground">{activeSection}</span>
            </div>
          ) : (
            <div className="space-y-2">
              <Textarea
                value={rawJson}
                onChange={(e) => setRawJson(e.target.value)}
                className="font-mono text-xs min-h-[300px]"
                placeholder='{"key": "value"}'
              />
              <Button size="sm">Save</Button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
