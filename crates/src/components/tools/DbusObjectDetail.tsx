import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Box, Wrench, FileText, Signal } from "lucide-react";
import type { DbusObjectTool } from "./dbus-tools-data";

interface Props {
  tool: DbusObjectTool;
  categoryColors: Record<string, string>;
}

export function DbusObjectDetail({ tool, categoryColors }: Props) {
  return (
    <ScrollArea className="h-full">
      <div className="p-6 space-y-6 max-w-2xl">
        {/* Header */}
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Box className={`h-4 w-4 ${categoryColors[tool.category] || "text-muted-foreground"}`} />
            <h2 className="text-base font-mono font-semibold text-foreground">{tool.name}</h2>
            <Badge variant="outline" className={`text-[10px] font-mono ${categoryColors[tool.category] || ""}`}>
              {tool.category}
            </Badge>
          </div>
          <p className="text-sm text-muted-foreground">{tool.description}</p>
          <div className="flex flex-col gap-0.5 text-[11px] font-mono text-muted-foreground/60">
            <span>Interface: <span className="text-foreground/70">{tool.interface}</span></span>
            <span>Path: <span className="text-foreground/70">{tool.dbusPath}</span></span>
          </div>
          <div className="flex flex-wrap gap-1 pt-1">
            {tool.tags.map((tag) => (
              <Badge key={tag} variant="secondary" className="text-[10px] font-mono">{tag}</Badge>
            ))}
          </div>
        </div>

        {/* Methods */}
        <div className="space-y-2">
          <h3 className="text-[10px] uppercase tracking-widest text-muted-foreground/60 font-semibold flex items-center gap-1.5">
            <Wrench className="h-3 w-3" /> Methods ({tool.methods.length})
          </h3>
          <div className="space-y-1.5">
            {tool.methods.map((m) => (
              <div key={m.name} className="rounded bg-muted px-3 py-2 space-y-0.5">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-mono font-medium text-foreground">{m.name}</span>
                  <code className="text-[10px] font-mono text-muted-foreground shrink-0">{m.signature}</code>
                </div>
                <p className="text-[11px] text-muted-foreground">{m.description}</p>
              </div>
            ))}
          </div>
        </div>

        {/* Properties */}
        <div className="space-y-2">
          <h3 className="text-[10px] uppercase tracking-widest text-muted-foreground/60 font-semibold flex items-center gap-1.5">
            <FileText className="h-3 w-3" /> Properties ({tool.properties.length})
          </h3>
          <div className="rounded border border-border overflow-hidden">
            {tool.properties.map((p, i) => (
              <div
                key={p.name}
                className={`flex items-center justify-between px-3 py-2 text-xs font-mono ${
                  i > 0 ? "border-t border-border" : ""
                }`}
              >
                <span className="text-foreground">{p.name}</span>
                <div className="flex items-center gap-2">
                  <span className="text-primary">{p.value}</span>
                  <span className="text-[10px] text-muted-foreground/50">({p.type})</span>
                  {p.access === "readwrite" && (
                    <Badge variant="outline" className="text-[9px] font-mono text-warning border-warning/30">rw</Badge>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Signals */}
        {tool.signals.length > 0 && (
          <div className="space-y-2">
            <h3 className="text-[10px] uppercase tracking-widest text-muted-foreground/60 font-semibold flex items-center gap-1.5">
              <Signal className="h-3 w-3" /> Signals ({tool.signals.length})
            </h3>
            <div className="space-y-1.5">
              {tool.signals.map((s) => (
                <div key={s.name} className="rounded bg-muted px-3 py-2 flex items-center justify-between">
                  <span className="text-xs font-mono text-foreground">{s.name}</span>
                  <code className="text-[10px] font-mono text-muted-foreground">{s.args}</code>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </ScrollArea>
  );
}
