import { useEffect, useMemo, useState, useCallback } from "react";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Skeleton } from "@/components/ui/skeleton";
import { Label } from "@/components/ui/label";
import { ChevronRight, ChevronDown, Sparkles, Search, LayoutGrid, LayoutList, Layers, ArrowLeft, Eye, EyeOff } from "lucide-react";
import { api } from "@/lib/api";

interface FieldInfo { name: string; control_type?: string; example?: unknown; description?: string; children?: FieldInfo[]; }
interface PluginEntry { category?: string; version?: string; fields?: FieldInfo[]; description?: string; }
type Catalog = Record<string, PluginEntry[]>;

interface UISpec {
  label: string;
  element: "button" | "toggle" | "input" | "select" | "nav" | "badge" | "section" | "insight";
  icon?: string;
  value?: unknown;
  options?: string[];
  children?: UISpec[];
  drillPath?: string[];
}

const GEMMA_PROMPT = (pluginId: string, fields: FieldInfo[], description: string) => `
You are an intelligent system analyst with deep knowledge of infrastructure, security, networking, and operations.

You are looking at a plugin called "${pluginId}": ${description}
Fields: ${fields.map(f => `${f.name} (${f.control_type ?? "unknown"})`).join(", ")}

Think freely using these 4 perspectives:
1. DATA: What types, ranges, validation rules matter? What numeric relationships are revealing?
2. SPATIAL: How should fields group and nest? What needs drill-down navigation for large datasets?
3. FLOW: What's the primary action? What field order makes sense? What needs confirmation?
4. AESTHETIC: What tone? Where does "danger" styling belong? Toggle vs checkbox?

Use those as conversation starters — surface the 2-3 things most worth an operator's attention.

Output ONLY valid JSON (no prose, no markdown fences):
{
  "insight": "1-2 sentences: what you noticed and why it matters",
  "elements": [{"label":"...", "element":"insight|toggle|input|select|nav|button|badge|section", "value":..., "options":[...], "children":[...], "drillPath":["parent","child"]}]
}`;

const ALL_CATEGORIES = ["all", "llm", "security", "network", "infrastructure", "data", "system", "hardware", "automation", "ui"];

const CATEGORY_COLORS: Record<string, string> = {
  llm: "bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-300",
  security: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300",
  network: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300",
  infrastructure: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300",
  data: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300",
  system: "bg-gray-100 text-gray-800 dark:bg-gray-900/30 dark:text-gray-300",
  hardware: "bg-orange-100 text-orange-800 dark:bg-orange-900/30 dark:text-orange-300",
  automation: "bg-indigo-100 text-indigo-800 dark:bg-indigo-900/30 dark:text-indigo-300",
  ui: "bg-pink-100 text-pink-800 dark:bg-pink-900/30 dark:text-pink-300",
};

function LiveControl({ spec, depth = 0, onDrill }: { spec: UISpec; depth?: number; onDrill?: (path: string[]) => void }) {
  const [expanded, setExpanded] = useState(depth < 2);
  const [val, setVal] = useState(spec.value ?? "");

  switch (spec.element) {
    case "insight":
      return <div className="rounded-md bg-primary/5 border border-primary/20 px-2.5 py-1.5 text-[10px] text-primary italic leading-relaxed">💬 {spec.label}</div>;
    case "toggle":
      return <div className="flex items-center justify-between py-1.5"><Label className="text-xs font-medium">{spec.label}</Label><Switch checked={!!val} onCheckedChange={setVal} className="scale-90" /></div>;
    case "input":
      return <div className="space-y-1 py-1"><Label className="text-xs font-medium">{spec.label}</Label><Input value={String(val)} onChange={e => setVal(e.target.value)} className="h-7 text-xs" placeholder={spec.label} /></div>;
    case "select":
      return <div className="space-y-1 py-1"><Label className="text-xs font-medium">{spec.label}</Label><select value={String(val)} onChange={e => setVal(e.target.value)} className="w-full h-7 text-xs border rounded px-2 bg-background">{(spec.options ?? []).map(o => <option key={o} value={o}>{o}</option>)}</select></div>;
    case "nav":
      return <button className="flex items-center justify-between w-full py-1.5 px-2 rounded hover:bg-muted text-xs group" onClick={() => onDrill?.(spec.drillPath ?? [spec.label])}><span className="font-medium">{spec.label}</span><ChevronRight className="w-3.5 h-3.5 text-muted-foreground group-hover:text-foreground" /></button>;
    case "button":
      return <Button size="sm" variant="outline" className="h-7 text-xs w-full mt-1">{spec.label}</Button>;
    case "badge":
      return <Badge variant="secondary" className="text-[10px]">{spec.label}</Badge>;
    case "section":
      return (
        <div className="border rounded-md overflow-hidden mt-2">
          <button className="flex items-center justify-between w-full px-3 py-2 bg-muted/50 hover:bg-muted text-xs font-semibold" onClick={() => setExpanded(x => !x)}>
            <span>{spec.label}</span>
            {expanded ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
          </button>
          {expanded && spec.children && <div className="px-3 py-2 space-y-1">{spec.children.map((c, i) => <LiveControl key={i} spec={c} depth={depth + 1} onDrill={onDrill} />)}</div>}
        </div>
      );
    default: return null;
  }
}

function GemmaCard({ pluginId, title, category, version, fields, description }: {
  pluginId: string; title: string; category: string; version?: string; fields: FieldInfo[]; description: string;
}) {
  const [specs, setSpecs] = useState<UISpec[] | null>(null);
  const [insight, setInsight] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [drillPath, setDrillPath] = useState<string[]>([]);
  const [showRaw, setShowRaw] = useState(false);

  const generate = useCallback(async () => {
    setLoading(true); setError(""); setSpecs(null); setInsight("");
    let text = "";
    try {
      const result = await api.chat.send("default", GEMMA_PROMPT(pluginId, fields, description), "antigravity", undefined);
      const raw = result as { content?: string; message?: string } | string;
      text = typeof raw === "string" ? raw : (raw?.content ?? raw?.message ?? "");
      const jsonMatch = text.match(/\{[\s\S]*\}/) ?? text.match(/\[[\s\S]*\]/);
      if (!jsonMatch) throw new Error(`No JSON found\n\nRaw: ${text.slice(0, 300)}`);
      let parsed: { insight?: string; elements?: UISpec[] } | UISpec[];
      try { parsed = JSON.parse(jsonMatch[0]); }
      catch { parsed = JSON.parse(jsonMatch[0].replace(/```json?/g, "").replace(/```/g, "").trim()); }
      const elems: UISpec[] = Array.isArray(parsed) ? parsed : ((parsed as { elements?: UISpec[] }).elements ?? []);
      const ins = (parsed as { insight?: string }).insight;
      if (ins) setInsight(ins);
      setSpecs(elems);
    } catch (e) {
      setError((e instanceof Error ? e.message : String(e)) + (text ? `\n\nRaw: ${text.slice(0, 300)}` : ""));
    } finally { setLoading(false); }
  }, [pluginId, fields, description]);

  const currentSpecs = useMemo(() => {
    if (!specs || drillPath.length === 0) return specs;
    let cur = specs;
    for (const seg of drillPath) {
      const next = cur.find(s => s.label === seg || s.drillPath?.includes(seg));
      if (next?.children) cur = next.children; else break;
    }
    return cur;
  }, [specs, drillPath]);

  return (
    <Card className="flex flex-col h-full">
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0"><Sparkles className="w-4 h-4 text-primary shrink-0" /><CardTitle className="text-sm leading-tight truncate">{title}</CardTitle></div>
          <Badge className={`shrink-0 text-[10px] px-1.5 ${CATEGORY_COLORS[category] ?? ""}`} variant="outline">{category}</Badge>
        </div>
        <div className="flex items-center gap-2 mt-1">
          <Badge variant="secondary" className="text-[10px]">{fields.length} fields</Badge>
          {version && <Badge variant="outline" className="text-[10px]">v{version}</Badge>}
        </div>
      </CardHeader>
      <CardContent className="flex-1 space-y-2 pb-2 min-h-0">
        {drillPath.length > 0 && (
          <div className="flex items-center gap-1 text-[10px] text-muted-foreground flex-wrap">
            <button className="hover:text-foreground flex items-center gap-0.5" onClick={() => setDrillPath([])}><ArrowLeft className="w-3 h-3" /> root</button>
            {drillPath.map((seg, i) => <span key={i} className="flex items-center gap-0.5"><ChevronRight className="w-2.5 h-2.5" /><button className="hover:text-foreground" onClick={() => setDrillPath(drillPath.slice(0, i + 1))}>{seg}</button></span>)}
          </div>
        )}
        {insight && <div className="rounded-md bg-primary/5 border border-primary/20 px-2.5 py-1.5 text-[10px] text-primary italic leading-relaxed">💬 {insight}</div>}
        {!specs && !loading && <p className="text-xs text-muted-foreground line-clamp-3 leading-relaxed">{description}</p>}
        {loading && <div className="space-y-2"><Skeleton className="h-6 w-full" /><Skeleton className="h-6 w-3/4" /><Skeleton className="h-6 w-5/6" /></div>}
        {error && <p className="text-[10px] text-destructive whitespace-pre-wrap">{error}</p>}
        {currentSpecs && !loading && (
          <ScrollArea className="max-h-48">
            {showRaw ? <pre className="text-[9px] text-muted-foreground whitespace-pre-wrap">{JSON.stringify(currentSpecs, null, 2)}</pre>
              : <div className="space-y-0.5">{currentSpecs.map((s, i) => <LiveControl key={i} spec={s} depth={0} onDrill={path => setDrillPath(prev => [...prev, ...path])} />)}</div>}
          </ScrollArea>
        )}
      </CardContent>
      <CardFooter className="pt-0 gap-2 flex-wrap">
        {specs && <button className="text-[10px] text-muted-foreground hover:text-foreground flex items-center gap-1" onClick={() => setShowRaw(x => !x)}>{showRaw ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}{showRaw ? "live" : "raw"}</button>}
        <Button variant={specs ? "ghost" : "outline"} size="sm" className="flex-1 h-7 text-xs gap-1" onClick={generate} disabled={loading}>
          <Sparkles className="w-3 h-3" />{specs ? "Regenerate" : "Generate UI"}
        </Button>
      </CardFooter>
    </Card>
  );
}

export default function GemmaUIPage() {
  const [catalog, setCatalog] = useState<Catalog>({});
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [activeTab, setActiveTab] = useState("all");
  const [layout, setLayout] = useState<"grid" | "list">("grid");

  useEffect(() => {
    let cancelled = false;
    fetch("/api/schema").then(r => r.ok ? r.json() : Promise.reject("non-ok"))
      .then((data: Catalog) => { if (!cancelled) { setCatalog(data); setLoading(false); } })
      .catch(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, []);

  const cards = useMemo(() => Object.entries(catalog).filter(([id]) => id !== "schema_renderer").map(([pluginId, entries]) => {
    const best = entries.find(e => (e.fields?.length ?? 0) > 0) ?? entries[0];
    if (!best) return null;
    const fields = best.fields ?? [];
    return { pluginId, title: pluginId.replace(/_/g, " ").replace(/\b\w/g, c => c.toUpperCase()), category: (best.category || "system").toLowerCase(), version: best.version, fields, description: best.description ?? `${fields.length} schema fields` };
  }).filter(Boolean) as Array<{ pluginId: string; title: string; category: string; version?: string; fields: FieldInfo[]; description: string }>, [catalog]);

  const usedCategories = useMemo(() => { const cats = new Set(cards.map(c => c.category)); return ALL_CATEGORIES.filter(c => c === "all" || cats.has(c)); }, [cards]);
  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    return cards.filter(c => { if (activeTab !== "all" && c.category !== activeTab) return false; if (q && !c.title.toLowerCase().includes(q) && !c.pluginId.toLowerCase().includes(q)) return false; return true; });
  }, [cards, search, activeTab]);

  if (loading) return (
    <div className="container mx-auto p-6 space-y-6">
      <h1 className="text-3xl font-bold flex items-center gap-2"><Sparkles className="w-7 h-7 text-primary" /> Gemma UI</h1>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
        {Array.from({ length: 8 }).map((_, i) => <Card key={i}><CardHeader><Skeleton className="h-5 w-32" /></CardHeader><CardContent className="space-y-2"><Skeleton className="h-6 w-full" /><Skeleton className="h-6 w-3/4" /></CardContent></Card>)}
      </div>
    </div>
  );

  return (
    <div className="container mx-auto p-6 space-y-6">
      <div className="flex items-start justify-between flex-wrap gap-4">
        <div>
          <h1 className="text-3xl font-bold flex items-center gap-2"><Sparkles className="w-7 h-7 text-primary" /> Gemma UI</h1>
          <p className="text-muted-foreground mt-1">Live UI generated from schema · Gemma surfaces what matters · drill-down for nested data</p>
        </div>
        <Badge variant="outline" className="gap-1"><Layers className="w-3.5 h-3.5" /> {filtered.length} plugins</Badge>
      </div>
      <div className="flex items-center gap-3 flex-wrap">
        <div className="relative flex-1 min-w-[220px] max-w-md">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <Input placeholder="Search plugins…" value={search} onChange={e => setSearch(e.target.value)} className="pl-8" />
        </div>
        <div className="flex border rounded-md overflow-hidden">
          <button onClick={() => setLayout("grid")} className={`p-2 transition-colors ${layout === "grid" ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:bg-muted"}`}><LayoutGrid className="w-4 h-4" /></button>
          <button onClick={() => setLayout("list")} className={`p-2 transition-colors ${layout === "list" ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:bg-muted"}`}><LayoutList className="w-4 h-4" /></button>
        </div>
      </div>
      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <ScrollArea className="pb-2">
          <TabsList className="h-auto flex-wrap justify-start gap-1 bg-transparent p-0">
            {usedCategories.map(cat => <TabsTrigger key={cat} value={cat} className="capitalize px-3 py-1.5 text-xs rounded-md">{cat}</TabsTrigger>)}
          </TabsList>
        </ScrollArea>
        <div className="mt-4">
          <ScrollArea className="h-[calc(100vh-340px)]">
            {filtered.length === 0 ? <div className="flex flex-col items-center justify-center py-20 text-muted-foreground"><Sparkles className="w-10 h-10 mb-3 opacity-30" /><p className="text-sm">No plugins match.</p></div>
              : layout === "grid" ? <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">{filtered.map(card => <GemmaCard key={card.pluginId} {...card} />)}</div>
              : <div className="space-y-2">{filtered.map(card => <Card key={card.pluginId} className="p-3"><div className="flex items-center gap-3"><Sparkles className="w-4 h-4 text-primary shrink-0" /><div className="flex-1 min-w-0"><div className="flex items-center gap-2 flex-wrap"><span className="font-semibold text-sm truncate">{card.title}</span><Badge className={`text-[10px] px-1.5 ${CATEGORY_COLORS[card.category] ?? ""}`} variant="outline">{card.category}</Badge></div><p className="text-xs text-muted-foreground mt-0.5 truncate">{card.description}</p></div><Button variant="outline" size="sm" className="h-7 text-xs" onClick={() => {}}>Open</Button></div></Card>)}</div>}
          </ScrollArea>
        </div>
      </Tabs>
    </div>
  );
}
