import { useEffect, useMemo, useState, useCallback, useRef } from "react";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Skeleton } from "@/components/ui/skeleton";
import { Sparkles, Search, Layers, Zap } from "lucide-react";
import { api } from "@/lib/api";

interface FieldInfo { name: string; control_type?: string; example?: unknown; description?: string; }
interface PluginEntry { category?: string; version?: string; fields?: FieldInfo[]; description?: string; }
type Catalog = Record<string, PluginEntry[]>;

const PROMPT = (pluginId: string, fields: FieldInfo[], description: string) =>
  `You are an intelligent system analyst reviewing a plugin called "${pluginId}" in a live infrastructure dashboard.

Here is what this plugin exposes:
${description}

Fields: ${fields.map(f => `${f.name} (${f.control_type ?? "unknown"})`).join(", ")}

Think freely. What would you want to know or do if you were the operator managing this right now?
What relationships between these fields are worth watching? What could go wrong that isn't obvious?
What would you surface as the single most important conversation starter for someone seeing this plugin for the first time?

Respond naturally — no JSON, no lists, just your honest perspective as a conversation starter.`;

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

// ── FreeCard ───────────────────────────────────────────────────────────────

function FreeCard({
  pluginId, title, category, version, fields, description, autoGenerate,
}: {
  pluginId: string; title: string; category: string; version?: string;
  fields: FieldInfo[]; description: string; autoGenerate?: boolean;
}) {
  const [response, setResponse] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const didAutoRef = useRef(false);

  const generate = useCallback(async () => {
    setLoading(true);
    setError("");
    setResponse("");
    try {
      const result = await api.chat.send("default", PROMPT(pluginId, fields, description), "antigravity", undefined);
      const raw = result as { content?: string; message?: string } | string;
      const text = typeof raw === "string" ? raw : (raw?.content ?? raw?.message ?? "");
      setResponse(text);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [pluginId, fields, description]);

  useEffect(() => {
    if (autoGenerate && !didAutoRef.current) {
      didAutoRef.current = true;
      generate();
    }
  }, [autoGenerate, generate]);

  return (
    <Card className="flex flex-col h-full">
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <Sparkles className="w-4 h-4 text-primary shrink-0" />
            <CardTitle className="text-sm leading-tight truncate">{title}</CardTitle>
          </div>
          <Badge className={`shrink-0 text-[10px] px-1.5 ${CATEGORY_COLORS[category] ?? ""}`} variant="outline">
            {category}
          </Badge>
        </div>
        <div className="flex items-center gap-2 mt-1">
          <Badge variant="secondary" className="text-[10px]">{fields.length} fields</Badge>
          {version && <Badge variant="outline" className="text-[10px]">v{version}</Badge>}
        </div>
      </CardHeader>

      <CardContent className="flex-1 pb-2 min-h-0">
        {loading && (
          <div className="space-y-2">
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-5/6" />
            <Skeleton className="h-3 w-4/5" />
            <Skeleton className="h-3 w-3/4" />
          </div>
        )}
        {error && <p className="text-[10px] text-destructive">{error}</p>}
        {!response && !loading && (
          <p className="text-xs text-muted-foreground line-clamp-3">{description}</p>
        )}
        {response && !loading && (
          <ScrollArea className="max-h-48">
            <p className="text-xs leading-relaxed text-foreground whitespace-pre-wrap">{response}</p>
          </ScrollArea>
        )}
      </CardContent>

      <CardFooter className="pt-0">
        <Button
          variant={response ? "ghost" : "outline"}
          size="sm"
          className="w-full h-7 text-xs gap-1"
          onClick={generate}
          disabled={loading}
        >
          <Sparkles className="w-3 h-3" />
          {response ? "Ask again" : "What does Gemma think?"}
        </Button>
      </CardFooter>
    </Card>
  );
}

// ── GemmaFreeUIPage ────────────────────────────────────────────────────────

export default function GemmaFreeUIPage() {
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

  const cards = useMemo(() => {
    return Object.entries(catalog)
      .filter(([id]) => id !== "schema_renderer")
      .map(([pluginId, entries]) => {
        const best = entries.find((e) => (e.fields?.length ?? 0) > 0) ?? entries[0];
        if (!best) return null;
        const fields = best.fields ?? [];
        return {
          pluginId,
          title: pluginId.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
          category: (best.category || "system").toLowerCase(),
          version: best.version,
          fields,
          description: best.description ?? `${fields.length} schema fields`,
        };
      }).filter(Boolean) as Array<{ pluginId: string; title: string; category: string; version?: string; fields: FieldInfo[]; description: string }>;
  }, [catalog]);

  const usedCategories = useMemo(() => {
    const cats = new Set(cards.map((c) => c.category));
    return ALL_CATEGORIES.filter((c) => c === "all" || cats.has(c));
  }, [cards]);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    return cards.filter((c) => {
      if (activeTab !== "all" && c.category !== activeTab) return false;
      if (q && !c.title.toLowerCase().includes(q) && !c.pluginId.toLowerCase().includes(q)) return false;
      return true;
    });
  }, [cards, search, activeTab]);

  if (loading) {
    return (
      <div className="container mx-auto p-6 space-y-6">
        <h1 className="text-3xl font-bold flex items-center gap-2"><Sparkles className="w-7 h-7 text-primary" /> Gemma Free UI</h1>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {Array.from({ length: 8 }).map((_, i) => <Card key={i}><CardHeader><Skeleton className="h-5 w-32" /></CardHeader><CardContent><Skeleton className="h-16 w-full" /></CardContent></Card>)}
        </div>
      </div>
    );
  }

  return (
    <div className="container mx-auto p-6 space-y-6">
      <div className="flex items-start justify-between flex-wrap gap-4">
        <div>
          <h1 className="text-3xl font-bold flex items-center gap-2">
            <Sparkles className="w-7 h-7 text-primary" /> Gemma Free UI
          </h1>
          <p className="text-muted-foreground mt-1">
            Gemma speaks freely — no structure, no format, just what it notices
          </p>
        </div>
        <div className="flex items-center gap-3">
          <Button
            size="sm"
            className="gap-1"
            onClick={() => setGenerateCount((n) => n + 10)}
          >
            <Zap className="w-4 h-4" /> Generate next 10
          </Button>
          <Badge variant="outline" className="gap-1">
            <Layers className="w-3.5 h-3.5" /> {filtered.length} plugins
          </Badge>
        </div>
      </div>

      <div className="relative max-w-md">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
        <Input placeholder="Search plugins…" value={search} onChange={(e) => setSearch(e.target.value)} className="pl-8" />
      </div>

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <ScrollArea className="pb-2">
          <TabsList className="h-auto flex-wrap justify-start gap-1 bg-transparent p-0">
            {usedCategories.map((cat) => (
              <TabsTrigger key={cat} value={cat} className="capitalize px-3 py-1.5 text-xs rounded-md">{cat}</TabsTrigger>
            ))}
          </TabsList>
        </ScrollArea>
        <div className="mt-4">
          <ScrollArea className="h-[calc(100vh-320px)]">
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
              {filtered.map((card, i) => (
                <FreeCard key={card.pluginId} {...card} autoGenerate={i < generateCount} />
              ))}
            </div>
          </ScrollArea>
        </div>
      </Tabs>
    </div>
  );
}
