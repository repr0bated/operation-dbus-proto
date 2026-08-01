import { useState, useMemo } from "react";
import { AppHeader } from "@/components/layout/AppHeader";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Search,
  ChevronRight,
  Box,
  Wrench,
  Signal,
  FileText,
  Folder,
  FolderOpen,
} from "lucide-react";
import { DbusObjectDetail } from "@/components/tools/DbusObjectDetail";
import type { DbusObjectTool } from "@/components/tools/dbus-tools-data";
import { mockDbusTools } from "@/components/tools/dbus-tools-data";

/* ── Tree builder ─────────────────────────────────────────── */
interface TreeNode {
  segment: string;
  fullPath: string;
  children: Map<string, TreeNode>;
  objects: DbusObjectTool[];
}

function buildTree(tools: DbusObjectTool[]): TreeNode {
  const root: TreeNode = { segment: "", fullPath: "/", children: new Map(), objects: [] };
  for (const tool of tools) {
    const parts = tool.dbusPath.split("/").filter(Boolean);
    let node = root;
    let path = "";
    for (const p of parts) {
      path += "/" + p;
      if (!node.children.has(p)) {
        node.children.set(p, { segment: p, fullPath: path, children: new Map(), objects: [] });
      }
      node = node.children.get(p)!;
    }
    node.objects.push(tool);
  }
  return root;
}

/* ── Category colors ──────────────────────────────────────── */
const categoryColors: Record<string, string> = {
  network: "text-accent",
  containers: "text-primary",
  audit: "text-warning",
  system: "text-muted-foreground",
  ai: "text-[hsl(var(--log-critical))]",
};

/* ── Tree node component ──────────────────────────────────── */
function TreeBranch({
  node,
  depth,
  selected,
  onSelect,
}: {
  node: TreeNode;
  depth: number;
  selected: DbusObjectTool | null;
  onSelect: (t: DbusObjectTool) => void;
}) {
  const [open, setOpen] = useState(depth < 3);
  const hasChildren = node.children.size > 0 || node.objects.length > 0;
  const childNodes = [...node.children.values()].sort((a, b) => a.segment.localeCompare(b.segment));

  if (!hasChildren) return null;

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className="flex items-center gap-1.5 w-full py-1 px-1 rounded hover:bg-muted/50 transition-colors group text-left">
        <ChevronRight
          className={`h-3 w-3 shrink-0 text-muted-foreground/50 transition-transform ${open ? "rotate-90" : ""}`}
        />
        {open ? (
          <FolderOpen className="h-3.5 w-3.5 shrink-0 text-muted-foreground/70" />
        ) : (
          <Folder className="h-3.5 w-3.5 shrink-0 text-muted-foreground/50" />
        )}
        <span className="text-xs font-mono text-muted-foreground group-hover:text-foreground truncate">
          {node.segment}
        </span>
        {node.objects.length > 0 && (
          <span className="text-[10px] text-muted-foreground/40 ml-auto font-mono">
            {node.objects.length}
          </span>
        )}
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="ml-3 border-l border-border/40 pl-2 space-y-0.5">
          {/* Objects at this node */}
          {node.objects.map((obj) => (
            <button
              key={obj.dbusPath}
              onClick={() => onSelect(obj)}
              className={`flex items-center gap-2 w-full py-1.5 px-2 rounded text-left transition-colors ${
                selected?.dbusPath === obj.dbusPath
                  ? "bg-primary/10 border border-primary/20"
                  : "hover:bg-muted/50 border border-transparent"
              }`}
            >
              <Box className={`h-3 w-3 shrink-0 ${categoryColors[obj.category] || "text-muted-foreground"}`} />
              <span className="text-xs font-mono font-medium text-foreground truncate">{obj.name}</span>
              <div className="ml-auto flex items-center gap-1.5 shrink-0">
                <span className="text-[9px] font-mono text-muted-foreground/50 flex items-center gap-0.5">
                  <Wrench className="h-2.5 w-2.5" />{obj.methods.length}
                </span>
                <span className="text-[9px] font-mono text-muted-foreground/50 flex items-center gap-0.5">
                  <Signal className="h-2.5 w-2.5" />{obj.signals.length}
                </span>
              </div>
            </button>
          ))}
          {/* Child branches */}
          {childNodes.map((child) => (
            <TreeBranch
              key={child.fullPath}
              node={child}
              depth={depth + 1}
              selected={selected}
              onSelect={onSelect}
            />
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

/* ── Main page ────────────────────────────────────────────── */
export default function ToolsPage() {
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<DbusObjectTool | null>(null);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    if (!q) return mockDbusTools;
    return mockDbusTools.filter(
      (t) =>
        t.name.toLowerCase().includes(q) ||
        t.description.toLowerCase().includes(q) ||
        t.category.toLowerCase().includes(q) ||
        t.interface.toLowerCase().includes(q) ||
        t.tags.some((tag) => tag.toLowerCase().includes(q))
    );
  }, [search]);

  const tree = useMemo(() => buildTree(filtered), [filtered]);

  const categories = useMemo(() => {
    const map = new Map<string, number>();
    mockDbusTools.forEach((t) => map.set(t.category, (map.get(t.category) || 0) + 1));
    return [...map.entries()].sort((a, b) => b[1] - a[1]);
  }, []);

  return (
    <>
      <AppHeader
        title="Tools"
        subtitle={`${mockDbusTools.length} D-Bus objects · tree view`}
      />
      <div className="flex-1 overflow-hidden flex">
        {/* Left: tree browser */}
        <div className="w-80 border-r border-border flex flex-col shrink-0">
          {/* Search + filters */}
          <div className="p-3 space-y-2 border-b border-border">
            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
              <Input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Filter objects…"
                className="pl-8 h-8 bg-muted border-border font-mono text-xs"
              />
            </div>
            <div className="flex flex-wrap gap-1">
              {categories.map(([cat, count]) => (
                <Badge
                  key={cat}
                  variant="outline"
                  className={`text-[9px] font-mono cursor-pointer hover:bg-accent/10 ${categoryColors[cat] || ""}`}
                  onClick={() => setSearch(search === cat ? "" : cat)}
                >
                  {cat} <span className="ml-0.5 text-muted-foreground/50">{count}</span>
                </Badge>
              ))}
            </div>
          </div>

          {/* Tree */}
          <ScrollArea className="flex-1">
            <div className="p-2 space-y-0.5">
              {filtered.length === 0 ? (
                <p className="text-xs text-muted-foreground font-mono py-6 text-center">
                  No objects match
                </p>
              ) : (
                [...tree.children.values()].map((child) => (
                  <TreeBranch
                    key={child.fullPath}
                    node={child}
                    depth={0}
                    selected={selected}
                    onSelect={setSelected}
                  />
                ))
              )}
            </div>
          </ScrollArea>
        </div>

        {/* Right: detail panel */}
        <div className="flex-1 overflow-hidden">
          {selected ? (
            <DbusObjectDetail tool={selected} categoryColors={categoryColors} />
          ) : (
            <div className="h-full flex items-center justify-center">
              <div className="text-center space-y-2">
                <Box className="h-8 w-8 text-muted-foreground/30 mx-auto" />
                <p className="text-sm text-muted-foreground/50 font-mono">
                  Select an object to introspect
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  );
}
