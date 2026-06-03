import { useEffect, useMemo, useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Skeleton } from "@/components/ui/skeleton";
import {
  BarChart, Bar, LineChart, Line, PieChart, Pie, Cell,
  RadarChart, Radar, PolarGrid, PolarAngleAxis,
  ResponsiveContainer, XAxis, YAxis, Tooltip, Legend,
} from "recharts";
import { Sparkles, Search, TrendingUp, PieChart as PieIcon, BarChart2, Activity } from "lucide-react";
import { api } from "@/lib/api";

// ── types ──────────────────────────────────────────────────────────────────

interface FieldInfo {
  name: string;
  control_type?: string;
  example?: unknown;
}

interface PluginEntry {
  category?: string;
  version?: string;
  fields?: FieldInfo[];
  description?: string;
}

type Catalog = Record<string, PluginEntry[]>;

type ChartType = "bar" | "line" | "pie" | "radar";

interface ChartSpec {
  type: ChartType;
  title: string;
  description: string;
  data: Array<Record<string, unknown>>;
  dataKey: string;
  nameKey: string;
  color: string;
}

// ── constants ──────────────────────────────────────────────────────────────

const GEMMA_PRESENTATION_PROMPT = `You are an intelligent system analyst with a gift for finding signal in data that humans miss.

Given a plugin schema, first ask yourself:
- What story does this data tell that isn't obvious from the field names alone?
- Are there correlations or tensions between fields that would be revealing if visualized together?
- What would surprise an operator if they saw this data plotted — what assumption would it challenge?
- If this plugin were misbehaving, what would the chart look like vs. healthy? Can you show both?
- What time dimension, distribution, or comparison would make the most useful dashboard widget?
- What would you want to see graphed if you were on-call and something went wrong?

Let those insights be your conversation starters. Don't chart everything — chart the one thing that would make an operator stop and say "huh, I didn't expect that." The chart is meant to provoke a question, not just display data.

Then express your findings as a chart. Rules for rendering:
- Choose chart type based on field types and semantics:
  - numeric fields with categories → bar chart
  - time-series or sequential data → line chart
  - proportions/distributions → pie chart
  - multi-dimensional comparison → radar chart
- Generate realistic-looking synthetic sample data based on field names and types
- For large datasets/trees with nested structure:
  - use drill-down bar charts (each bar is clickable to show sub-breakdown)
  - add a "drillable" flag to indicate sub-data is available
- data array should have 5-8 data points
- colors should be semantically appropriate (red for errors, green for healthy, blue for network, etc)

Output format (JSON only, no prose):
{
  "insight": "1-2 sentences — your conversation starter: what you noticed and why this chart reveals it",
  "type": "bar|line|pie|radar",
  "title": "...",
  "description": "...",
  "data": [{"name": "...", "value": 42, ...}],
  "dataKey": "value",
  "nameKey": "name",
  "color": "#hex"
}`;

const CHART_COLORS = [
  "#6366f1", "#22c55e", "#f59e0b", "#ec4899",
  "#14b8a6", "#f97316", "#8b5cf6", "#3b82f6",
];

const ALL_CATEGORIES = [
  "all", "llm", "security", "network", "infrastructure",
  "data", "system", "hardware", "automation",
];

// ── ChartRenderer ──────────────────────────────────────────────────────────

function ChartRenderer({ spec }: { spec: ChartSpec }) {
  switch (spec.type) {
    case "bar":
      return (
        <ResponsiveContainer width="100%" height={180}>
          <BarChart data={spec.data} margin={{ top: 4, right: 8, bottom: 4, left: -20 }}>
            <XAxis dataKey={spec.nameKey} tick={{ fontSize: 9 }} />
            <YAxis tick={{ fontSize: 9 }} />
            <Tooltip contentStyle={{ fontSize: 10 }} />
            <Bar dataKey={spec.dataKey} fill={spec.color} radius={[3, 3, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      );
    case "line":
      return (
        <ResponsiveContainer width="100%" height={180}>
          <LineChart data={spec.data} margin={{ top: 4, right: 8, bottom: 4, left: -20 }}>
            <XAxis dataKey={spec.nameKey} tick={{ fontSize: 9 }} />
            <YAxis tick={{ fontSize: 9 }} />
            <Tooltip contentStyle={{ fontSize: 10 }} />
            <Line type="monotone" dataKey={spec.dataKey} stroke={spec.color} dot={false} strokeWidth={2} />
          </LineChart>
        </ResponsiveContainer>
      );
    case "pie":
      return (
        <ResponsiveContainer width="100%" height={180}>
          <PieChart>
            <Pie
              data={spec.data}
              dataKey={spec.dataKey}
              nameKey={spec.nameKey}
              cx="50%"
              cy="50%"
              outerRadius={70}
              label={({ name, percent }) => `${name} ${(percent * 100).toFixed(0)}%`}
              labelLine={false}
            >
              {spec.data.map((_, i) => (
                <Cell key={i} fill={CHART_COLORS[i % CHART_COLORS.length]} />
              ))}
            </Pie>
            <Tooltip contentStyle={{ fontSize: 10 }} />
          </PieChart>
        </ResponsiveContainer>
      );
    case "radar":
      return (
        <ResponsiveContainer width="100%" height={180}>
          <RadarChart data={spec.data}>
            <PolarGrid />
            <PolarAngleAxis dataKey={spec.nameKey} tick={{ fontSize: 9 }} />
            <Radar dataKey={spec.dataKey} stroke={spec.color} fill={spec.color} fillOpacity={0.3} />
            <Tooltip contentStyle={{ fontSize: 10 }} />
          </RadarChart>
        </ResponsiveContainer>
      );
    default:
      return null;
  }
}

// ── PresentationCard ───────────────────────────────────────────────────────

function PresentationCard({
  pluginId,
  title,
  category,
  version,
  fields,
  description,
}: {
  pluginId: string;
  title: string;
  category: string;
  version?: string;
  fields: FieldInfo[];
  description: string;
}) {
  const [spec, setSpec] = useState<ChartSpec | null>(null);
  const [insight, setInsight] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [chartType, setChartType] = useState<ChartType | null>(null);

  const generate = useCallback(async (overrideType?: ChartType) => {
    setLoading(true);
    setError("");
    try {
      const payload = JSON.stringify({ pluginId, fields, description }, null, 2);
      const typeHint = overrideType ? `\nForce chart type: ${overrideType}` : "";
      const prompt = `${GEMMA_PRESENTATION_PROMPT}${typeHint}\n\nSchema:\n${payload}`;
      const result = await api.chat.send("default", prompt, "antigravity", undefined);
      const raw = result as { content?: string; message?: string } | string;
      const text = typeof raw === "string" ? raw : (raw?.content ?? raw?.message ?? "");
      const jsonMatch = text.match(/\{[\s\S]*\}/);
      if (!jsonMatch) throw new Error("No JSON in response");
      const parsed: ChartSpec & { insight?: string } = JSON.parse(jsonMatch[0]);
      if (parsed.insight) setInsight(parsed.insight);
      setSpec(parsed);
      setChartType(parsed.type);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [pluginId, fields, description]);

  const chartIcon = (t: ChartType) => {
    const icons: Record<ChartType, React.ReactNode> = {
      bar: <BarChart2 className="w-3 h-3" />,
      line: <Activity className="w-3 h-3" />,
      pie: <PieIcon className="w-3 h-3" />,
      radar: <TrendingUp className="w-3 h-3" />,
    };
    return icons[t];
  };

  return (
    <Card className="flex flex-col h-full">
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <CardTitle className="text-sm truncate">{title}</CardTitle>
          <Badge className={`shrink-0 text-[10px]`} variant="outline">{category}</Badge>
        </div>
        <div className="flex items-center gap-2 mt-1">
          <Badge variant="secondary" className="text-[10px]">{fields.length} fields</Badge>
          {version && <Badge variant="outline" className="text-[10px]">v{version}</Badge>}
        </div>
      </CardHeader>

      <CardContent className="flex-1 pb-2">
        {loading && (
          <div className="space-y-2">
            <Skeleton className="h-[180px] w-full" />
          </div>
        )}

        {error && <p className="text-[10px] text-destructive">{error}</p>}

        {!spec && !loading && (
          <p className="text-xs text-muted-foreground line-clamp-3">{description}</p>
        )}

        {insight && (
          <div className="rounded-md bg-primary/5 border border-primary/20 px-2.5 py-1.5 text-[10px] text-primary italic leading-relaxed mb-2">
            💬 {insight}
          </div>
        )}

        {spec && !loading && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <p className="text-[10px] text-muted-foreground truncate">{spec.description}</p>
              {/* chart type switcher */}
              <div className="flex gap-1">
                {(["bar", "line", "pie", "radar"] as ChartType[]).map((t) => (
                  <button
                    key={t}
                    onClick={() => generate(t)}
                    className={`p-1 rounded text-[10px] transition-colors ${
                      chartType === t
                        ? "bg-primary text-primary-foreground"
                        : "text-muted-foreground hover:bg-muted"
                    }`}
                    title={t}
                  >
                    {chartIcon(t)}
                  </button>
                ))}
              </div>
            </div>
            <ChartRenderer spec={spec} />
          </div>
        )}
      </CardContent>

      <div className="px-6 pb-4">
        <Button
          variant={spec ? "ghost" : "outline"}
          size="sm"
          className="w-full h-7 text-xs gap-1"
          onClick={() => generate()}
          disabled={loading}
        >
          <Sparkles className="w-3 h-3" />
          {spec ? "Regenerate" : "Generate Chart"}
        </Button>
      </div>
    </Card>
  );
}

// ── GemmaPresentationPage ──────────────────────────────────────────────────

export default function GemmaPresentationPage() {
  const [catalog, setCatalog] = useState<Catalog>({});
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [activeTab, setActiveTab] = useState("all");

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
      })
      .filter(Boolean) as Array<{
        pluginId: string; title: string; category: string;
        version?: string; fields: FieldInfo[]; description: string;
      }>;
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
        <h1 className="text-3xl font-bold flex items-center gap-2">
          <TrendingUp className="w-7 h-7 text-primary" /> Gemma Presentation
        </h1>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <Card key={i}>
              <CardHeader><Skeleton className="h-5 w-32" /></CardHeader>
              <CardContent><Skeleton className="h-[180px] w-full" /></CardContent>
            </Card>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="container mx-auto p-6 space-y-6">
      <div className="flex items-start justify-between flex-wrap gap-4">
        <div>
          <h1 className="text-3xl font-bold flex items-center gap-2">
            <TrendingUp className="w-7 h-7 text-primary" /> Gemma Presentation
          </h1>
          <p className="text-muted-foreground mt-1">
            Charts and graphs generated from schema · drill-down for large datasets
          </p>
        </div>
        <Badge variant="outline">{filtered.length} plugins</Badge>
      </div>

      <div className="relative max-w-md">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
        <Input
          placeholder="Search plugins…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="pl-8"
        />
      </div>

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <ScrollArea className="pb-2">
          <TabsList className="h-auto flex-wrap justify-start gap-1 bg-transparent p-0">
            {usedCategories.map((cat) => (
              <TabsTrigger key={cat} value={cat} className="capitalize px-3 py-1.5 text-xs rounded-md">
                {cat}
              </TabsTrigger>
            ))}
          </TabsList>
        </ScrollArea>

        <div className="mt-4">
          <ScrollArea className="h-[calc(100vh-300px)]">
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
              {filtered.map((card) => (
                <PresentationCard key={card.pluginId} {...card} />
              ))}
            </div>
          </ScrollArea>
        </div>
      </Tabs>
    </div>
  );
}
