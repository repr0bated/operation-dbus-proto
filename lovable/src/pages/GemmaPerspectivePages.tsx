// ── Shared infrastructure for the 4 Gemma Perspective pages ──────────────
// Each page is exported individually and uses the same FreeCard pattern.

import { useEffect, useMemo, useState, useCallback, useRef } from "react";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Skeleton } from "@/components/ui/skeleton";
import { Hash, LayoutDashboard, GitBranch, Palette, Search, Layers, Zap } from "lucide-react";
import { api } from "@/lib/api";

interface FieldInfo { name: string; control_type?: string; example?: unknown; description?: string; }
interface PluginEntry { category?: string; version?: string; fields?: FieldInfo[]; description?: string; }
type Catalog = Record<string, PluginEntry[]>;

const CATEGORY_COLORS: Record<string, string> = {
  llm: "bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-300",
  security: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300",
  network: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300",
  infrastructure: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300",
  data: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300",
  system: "bg-gray-100 text-gray-800 dark:bg-gray-900/30 dark:text-gray-300",
  hardware: "bg-orange-100 text-orange-800 dark:bg-orange-900/30 dark:text-orange-300",
  automation: "bg-indigo-100 text-indigo-800 dark:bg-indigo-900/30 dark:text-indigo-300",
};

const ALL_CATEGORIES = ["all", "llm", "security", "network", "infrastructure", "data", "system", "hardware", "automation", "ui"];

// ── Prompts ────────────────────────────────────────────────────────────────

const PROMPTS = {
  data: (pluginId: string, fields: FieldInfo[], description: string) =>
    `You are analyzing the DATA/NUMERIC perspective of a plugin called "${pluginId}": ${description}

Fields: ${fields.map(f => `${f.name} (${f.control_type ?? "unknown"})`).join(", ")}

Focus only on the data and numeric dimension:
- What are the exact types, ranges, and constraints on each field?
- Which fields are numeric and what are their meaningful ranges — what's normal vs alarming?
- What validation rules should exist (min/max, required, format)?
- Are there any fields where the numeric value has non-obvious meaning (e.g. 0 means disabled, -1 means unlimited)?
- What data relationships or ratios between numeric fields would be most revealing?

Respond as a conversation starter — what would you want an operator to know about the data shape of this plugin? No JSON, no lists, just your honest numeric/structural perspective.`,

  spatial: (pluginId: string, fields: FieldInfo[], description: string) =>
    `You are analyzing the SPATIAL/LAYOUT perspective of a plugin called "${pluginId}": ${description}

Fields: ${fields.map(f => `${f.name} (${f.control_type ?? "unknown"})`).join(", ")}

Focus only on the spatial and layout dimension:
- How should these fields be grouped and arranged on screen?
- Which fields belong together visually and which should be separated?
- What should be prominent/large vs tucked away/collapsed?
- Are there natural hierarchies or trees in this data that suggest drill-down navigation?
- What would clutter the interface vs what should always be visible at a glance?
- If this were a physical control panel, what would be the layout?

Respond as a conversation starter about how this plugin's data wants to be arranged spatially. No JSON, no lists, just your spatial reasoning.`,

  flow: (pluginId: string, fields: FieldInfo[], description: string) =>
    `You are analyzing the USER FLOW/INTERACTION perspective of a plugin called "${pluginId}": ${description}

Fields: ${fields.map(f => `${f.name} (${f.control_type ?? "unknown"})`).join(", ")}

Focus only on the interaction and user flow dimension:
- What order should fields be presented in?
- Which inputs must come before others (dependencies, prerequisites)?
- What is the primary call-to-action — what does the operator most likely want to DO with this plugin?
- Which fields are read-only status vs writable controls?
- What confirmation steps or warnings should gate destructive actions?
- What would a new operator do first, second, third with this plugin?

Respond as a conversation starter about the natural flow of interacting with this plugin. No JSON, no lists, just your flow/interaction perspective.`,

  aesthetic: (pluginId: string, fields: FieldInfo[], description: string) =>
    `You are analyzing the CONTEXT/AESTHETIC perspective of a plugin called "${pluginId}": ${description}

Fields: ${fields.map(f => `${f.name} (${f.control_type ?? "unknown"})`).join(", ")}

Focus only on semantic meaning and aesthetic choices:
- What is the emotional/operational tone of this plugin — routine monitoring, emergency response, configuration?
- Which fields deserve danger/warning styling and why?
- Where would a toggle be better than a checkbox? A slider better than a number input?
- What icons or visual metaphors would make this plugin's purpose immediately clear?
- What color coding would help operators make faster decisions?
- What would this plugin look like if it were designed for experts vs beginners?

Respond as a conversation starter about the semantic and aesthetic character of this plugin. No JSON, no lists, just your contextual/aesthetic perspective.`,
};

// ── PerspectiveCard ────────────────────────────────────────────────────────

function PerspectiveCard({
  pluginId, title, category, version, fields, description, autoGenerate,
  promptFn, buttonLabel,
}: {
  pluginId: string; title: string; category: string; version?: string;
  fields: FieldInfo[]; description: string; autoGenerate?: boolean;
  promptFn: (id: string, fields: FieldInfo[], desc: string) => string;
  buttonLabel: string;
}) {
  const [response, setResponse] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const didAutoRef = useRef(false);

  const generate = useCallback(async () => {
    setLoading(true); setError(""); setResponse("");
    try {
      const result = await api.chat.send("default", promptFn(pluginId, fields, description), "antigravity", undefined);
      const raw = result as { content?: string; message?: string } | string;
      setResponse(typeof raw === "string" ? raw : (raw?.content ?? raw?.message ?? ""));
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setLoading(false); }
  }, [pluginId, fields, description, promptFn]);

  useEffect(() => {
    if (autoGenerate && !didAutoRef.current) { didAutoRef.current = true; generate(); }
  }, [autoGenerate, generate]);

  return (
    <Card className="flex flex-col h-full">
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <CardTitle className="text-sm leading-tight truncate">{title}</CardTitle>
          <Badge className={`shrink-0 text-[10px] px-1.5 ${CATEGORY_COLORS[category] ?? ""}`} variant="outline">{category}</Badge>
        </div>
        <div className="flex items-center gap-2 mt-1">
          <Badge variant="secondary" className="text-[10px]">{fields.length} fields</Badge>
          {version && <Badge variant="outline" className="text-[10px]">v{version}</Badge>}
        </div>
      </CardHeader>
      <CardContent className="flex-1 pb-2 min-h-0">
        {loading && <div className="space-y-2"><Skeleton className="h-3 w-full" /><Skeleton className="h-3 w-5/6" /><Skeleton className="h-3 w-4/5" /><Skeleton className="h-3 w-3/4" /></div>}
        {error && <p className="text-[10px] text-destructive">{error}</p>}
        {!response && !loading && <p className="text-xs text-muted-foreground line-clamp-3">{description}</p>}
        {response && !loading && (
          <ScrollArea className="max-h-52">
            <p className="text-xs leading-relaxed whitespace-pre-wrap">{response}</p>
          </ScrollArea>
        )}
      </CardContent>
      <CardFooter className="pt-0">
        <Button variant={response ? "ghost" : "outline"} size="sm" className="w-full h-7 text-xs gap-1" onClick={generate} disabled={loading}>
          {buttonLabel}
        </Button>
      </CardFooter>
    </Card>
  );
}

// ── Shared page factory ────────────────────────────────────────────────────

function PerspectivePage({
  title, subtitle, icon: Icon, promptFn, buttonLabel, iconColor,
}: {
  title: string; subtitle: string; icon: React.ComponentType<{ className?: string }>;
  promptFn: (id: string, fields: FieldInfo[], desc: string) => string;
  buttonLabel: string; iconColor: string;
}) {
  const [catalog, setCatalog] = useState<Catalog>({});
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [activeTab, setActiveTab] = useState("all");
  const [generateCount, setGenerateCount] = useState(0);

  useEffect(() => {
    let cancelled = false;
    fetch("/api/schema")
      .then((r) => r.ok ? r.json() : Promise.reject("non-ok"))
      .then((data: Catalog) => { if (!cancelled) { setCatalog(data); setLoading(false); } })
      .catch(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, []);

  const cards = useMemo(() => Object.entries(catalog).filter(([id]) => id !== "schema_renderer").map(([pluginId, entries]) => {
    const best = entries.find((e) => (e.fields?.length ?? 0) > 0) ?? entries[0];
    if (!best) return null;
    const fields = best.fields ?? [];
    return { pluginId, title: pluginId.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()), category: (best.category || "system").toLowerCase(), version: best.version, fields, description: best.description ?? `${fields.length} schema fields` };
  }).filter(Boolean) as Array<{ pluginId: string; title: string; category: string; version?: string; fields: FieldInfo[]; description: string }>, [catalog]);

  const usedCategories = useMemo(() => { const cats = new Set(cards.map((c) => c.category)); return ALL_CATEGORIES.filter((c) => c === "all" || cats.has(c)); }, [cards]);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    return cards.filter((c) => {
      if (activeTab !== "all" && c.category !== activeTab) return false;
      if (q && !c.title.toLowerCase().includes(q) && !c.pluginId.toLowerCase().includes(q)) return false;
      return true;
    });
  }, [cards, search, activeTab]);

  if (loading) return (
    <div className="container mx-auto p-6 space-y-6">
      <h1 className="text-3xl font-bold flex items-center gap-2"><Icon className={`w-7 h-7 ${iconColor}`} />{title}</h1>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
        {Array.from({ length: 8 }).map((_, i) => <Card key={i}><CardHeader><Skeleton className="h-5 w-32" /></CardHeader><CardContent><Skeleton className="h-16 w-full" /></CardContent></Card>)}
      </div>
    </div>
  );

  return (
    <div className="container mx-auto p-6 space-y-6">
      <div className="flex items-start justify-between flex-wrap gap-4">
        <div>
          <h1 className="text-3xl font-bold flex items-center gap-2"><Icon className={`w-7 h-7 ${iconColor}`} />{title}</h1>
          <p className="text-muted-foreground mt-1">{subtitle}</p>
        </div>
        <div className="flex items-center gap-3">
          <Button size="sm" className="gap-1" onClick={() => setGenerateCount((n) => n + 10)}>
            <Zap className="w-4 h-4" /> Generate next 10
          </Button>
          <Badge variant="outline" className="gap-1"><Layers className="w-3.5 h-3.5" />{filtered.length} plugins</Badge>
        </div>
      </div>
      <div className="relative max-w-md">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
        <Input placeholder="Search plugins…" value={search} onChange={(e) => setSearch(e.target.value)} className="pl-8" />
      </div>
      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <ScrollArea className="pb-2">
          <TabsList className="h-auto flex-wrap justify-start gap-1 bg-transparent p-0">
            {usedCategories.map((cat) => <TabsTrigger key={cat} value={cat} className="capitalize px-3 py-1.5 text-xs rounded-md">{cat}</TabsTrigger>)}
          </TabsList>
        </ScrollArea>
        <div className="mt-4">
          <ScrollArea className="h-[calc(100vh-320px)]">
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
              {filtered.map((card, i) => (
                <PerspectiveCard key={card.pluginId} {...card} autoGenerate={i < generateCount} promptFn={promptFn} buttonLabel={buttonLabel} />
              ))}
            </div>
          </ScrollArea>
        </div>
      </Tabs>
    </div>
  );
}

// ── 4 exported pages ───────────────────────────────────────────────────────

export function GemmaDataPage() {
  return <PerspectivePage title="Data Perspective" subtitle="Gemma reasons about types, ranges, validation, and numeric relationships" icon={Hash} iconColor="text-blue-500" promptFn={PROMPTS.data} buttonLabel="Analyze data shape" />;
}

export function GemmaSpatialPage() {
  return <PerspectivePage title="Spatial Perspective" subtitle="Gemma reasons about grouping, hierarchy, layout, and visual weight" icon={LayoutDashboard} iconColor="text-green-500" promptFn={PROMPTS.spatial} buttonLabel="Reason about layout" />;
}

export function GemmaFlowPage() {
  return <PerspectivePage title="Flow Perspective" subtitle="Gemma reasons about interaction order, dependencies, and calls-to-action" icon={GitBranch} iconColor="text-orange-500" promptFn={PROMPTS.flow} buttonLabel="Map user flow" />;
}

export function GemmaAestheticPage() {
  return <PerspectivePage title="Aesthetic Perspective" subtitle="Gemma reasons about semantic meaning, tone, and component choices" icon={Palette} iconColor="text-pink-500" promptFn={PROMPTS.aesthetic} buttonLabel="Read the context" />;
}
