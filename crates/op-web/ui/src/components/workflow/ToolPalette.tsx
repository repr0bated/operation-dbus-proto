/**
 * Left-pane tool palette for the Workflow Builder.
 * Tools are draggable into the React Flow canvas.
 */
import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Search, Wrench, GripVertical } from "lucide-react";
import { cn } from "@/lib/utils";
import type { Tool } from "@/types/api";

interface ToolPaletteProps {
  tools: Tool[];
  className?: string;
}

export function ToolPalette({ tools, className }: ToolPaletteProps) {
  const [search, setSearch] = useState("");

  const filtered = tools.filter((t) =>
    !search || t.name.toLowerCase().includes(search.toLowerCase()) || t.category.toLowerCase().includes(search.toLowerCase())
  );

  const grouped = filtered.reduce<Record<string, Tool[]>>((acc, t) => {
    (acc[t.category] ??= []).push(t);
    return acc;
  }, {});

  const onDragStart = (e: React.DragEvent, tool: Tool) => {
    e.dataTransfer.setData("application/workflow-tool", JSON.stringify(tool));
    e.dataTransfer.effectAllowed = "move";
  };

  return (
    <div className={cn("flex flex-col h-full", className)}>
      <div className="px-3 py-2 border-b border-border">
        <div className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider mb-2">Tool Palette</div>
        <div className="relative">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
          <Input
            placeholder="Search tools..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-7 pl-7 text-xs bg-secondary border-none"
          />
        </div>
      </div>
      <div className="flex-1 overflow-auto p-2 space-y-3">
        {Object.entries(grouped).map(([cat, catTools]) => (
          <div key={cat}>
            <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider px-1 mb-1">{cat}</div>
            <div className="space-y-1">
              {catTools.map((tool) => (
                <div
                  key={tool.id}
                  draggable
                  onDragStart={(e) => onDragStart(e, tool)}
                  className="flex items-center gap-2 px-2 py-1.5 rounded-md border border-transparent hover:border-border hover:bg-muted/20 cursor-grab active:cursor-grabbing transition-colors group"
                >
                  <GripVertical className="h-3 w-3 text-muted-foreground/50 group-hover:text-muted-foreground shrink-0" />
                  <Wrench className="h-3 w-3 text-primary/70 shrink-0" />
                  <div className="flex-1 min-w-0">
                    <div className="text-xs font-medium text-foreground truncate">{tool.name}</div>
                    <div className="text-[10px] text-muted-foreground truncate">{tool.description}</div>
                  </div>
                  <Badge variant="outline" className="text-[9px] shrink-0">{tool.source}</Badge>
                </div>
              ))}
            </div>
          </div>
        ))}
        {filtered.length === 0 && (
          <div className="text-xs text-muted-foreground text-center py-4">No tools found.</div>
        )}
      </div>
    </div>
  );
}
