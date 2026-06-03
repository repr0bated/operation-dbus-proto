import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Skeleton } from "@/components/ui/skeleton";
import { Label } from "@/components/ui/label";
import { Eye, LayoutGrid, LayoutList, Puzzle, Search } from "lucide-react";

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
  render_config?: {
    max_fields_per_card?: number;
  };
}

type Catalog = Record<string, PluginEntry[]>;

// ── constants ──────────────────────────────────────────────────────────────

const ALL_CATEGORIES = [
  "all",
  "llm",
  "security",
  "network",
  "infrastructure",
  "data",
  "system",
  "hardware",
  "automation",
  "ui",
  "storage",
  "monitoring",
  "messaging",
  "crypto",
  "virtualization",
  "devtools",
];

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
  storage: "bg-teal-100 text-teal-800 dark:bg-teal-900/30 dark:text-teal-300",
  monitoring: "bg-cyan-100 text-cyan-800 dark:bg-cyan-900/30 dark:text-cyan-300",
  messaging: "bg-violet-100 text-violet-800 dark:bg-violet-900/30 dark:text-violet-300",
  crypto: "bg-rose-100 text-rose-800 dark:bg-rose-900/30 dark:text-rose-300",
  virtualization: "bg-lime-100 text-lime-800 dark:bg-lime-900/30 dark:text-lime-300",
  devtools: "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-300",
};

const CONTROL_COLORS: Record<string, string> = {
  string: "#60a5fa",
  number: "#34d399",
  boolean: "#fbbf24",
  enum: "#c084fc",
  object: "#f472b6",
  array: "#22d3ee",
};

const DEFAULT_CONTROL_COLOR = "#94a3b8";

// ── helpers ────────────────────────────────────────────────────────────────

function titleCase(id: string): string {
  return id.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

function getControlColor(type?: string): string {
  const key = (type || "string").toLowerCase();
  return CONTROL_COLORS[key] ?? DEFAULT_CONTROL_COLOR;
}

// ── MiniFieldPreview ───────────────────────────────────────────────────────

function MiniFieldPreview({ field }: { field: FieldInfo }) {
  return (
    <div className="flex items-center gap-2 text-xs py-0.5">
      <span
        className="w-1.5 h-1.5 rounded-full shrink-0"
        style={{ backgroundColor: getControlColor(field.control_type) }}
      />
      <span className="font-medium text-foreground truncate">{field.name}</span>
      <span className="text-muted-foreground truncate ml-auto">
        {(field.control_type || "string").toLowerCase()}
      </span>
    </div>
  );
}

// ── SkeletonCard ───────────────────────────────────────────────────────────

function SkeletonCard() {
  return (
    <Card>
      <CardHeader className="pb-2">
        <Skeleton className="h-5 w-32" />
      </CardHeader>
      <CardContent className="space-y-2">
        <Skeleton className="h-3 w-20" />
        <Skeleton className="h-3 w-full" />
        <Skeleton className="h-3 w-3/4" />
        <Skeleton className="h-3 w-1/2" />
      </CardContent>
      <CardFooter>
        <Skeleton className="h-8 w-full" />
      </CardFooter>
    </Card>
  );
}

// ── SchemaGalleryPage ──────────────────────────────────────────────────────

export default function SchemaGalleryPage() {
  const navigate = useNavigate();
  const [catalog, setCatalog] = useState<Catalog>({});
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [activeTab, setActiveTab] = useState("all");
  const [layout, setLayout] = useState<"grid" | "list">("grid");
  const [maxPreviewFields, setMaxPreviewFields] = useState(5);

  // ── fetch schema catalog ───────────────────────────────────────────────

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch("/api/schema");
        if (!res.ok) throw new Error("non-ok");
        const data: Catalog = await res.json();

        const rendererEntries = data["schema_renderer"];
        if (Array.isArray(rendererEntries)) {
          for (const entry of rendererEntries) {
            const max = entry?.render_config?.max_fields_per_card;
            if (typeof max === "number" && max > 0) {
              setMaxPreviewFields(max);
              break;
            }
          }
        }

        if (!cancelled) {
          setCatalog(data);
          setLoading(false);
        }
      } catch {
        if (!cancelled) setLoading(false);
      }
    }
    load();
    return () => {
      cancelled = true;
    };
  }, []);

  // ── derive cards from catalog ──────────────────────────────────────────

  const cards = useMemo(() => {
    const result: Array<{
      pluginId: string;
      title: string;
      description: string;
      category: string;
      version?: string;
      fieldCount: number;
      previewFields: FieldInfo[];
    }> = [];

    for (const [pluginId, entries] of Object.entries(catalog)) {
      if (pluginId === "schema_renderer") continue;

      let best: PluginEntry | undefined;
      for (const e of entries) {
        const len = e.fields?.length ?? 0;
        if (len > 0) {
          best = e;
          break;
        }
      }
      if (!best) best = entries[0];
      if (!best) continue;

      const category = (best.category || "system").toLowerCase();
      const fields = best.fields ?? [];
      result.push({
        pluginId,
        title: titleCase(pluginId),
        description: best.description ?? `${fields.length} schema fields`,
        category,
        version: best.version,
        fieldCount: fields.length,
        previewFields: fields.slice(0, maxPreviewFields),
      });
    }

    return result;
  }, [catalog, maxPreviewFields]);

  // ── filter logic ───────────────────────────────────────────────────────

  const usedCategories = useMemo(() => {
    const cats = new Set(cards.map((c) => c.category));
    return ALL_CATEGORIES.filter((c) => c === "all" || cats.has(c));
  }, [cards]);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    return cards.filter((c) => {
      if (activeTab !== "all" && c.category !== activeTab) return false;
      if (
        q &&
        !c.title.toLowerCase().includes(q) &&
        !c.pluginId.toLowerCase().includes(q) &&
        !c.description.toLowerCase().includes(q)
      )
        return false;
      return true;
    });
  }, [cards, search, activeTab]);

  // ── loading skeleton ───────────────────────────────────────────────────

  if (loading) {
    return (
      <div className="container mx-auto p-6 space-y-6">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Schema Gallery</h1>
          <p className="text-muted-foreground mt-1">Auto-generated from D-Bus projection</p>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <SkeletonCard key={i} />
          ))}
        </div>
      </div>
    );
  }

  // ── render ─────────────────────────────────────────────────────────────

  return (
    <div className="container mx-auto p-6 space-y-6">
      {/* header */}
      <div className="flex items-start justify-between flex-wrap gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Schema Gallery</h1>
          <p className="text-muted-foreground mt-1">
            {cards.length} elements from {Object.keys(catalog).length} plugins
          </p>
        </div>
        <Badge variant="outline" className="gap-1">
          <Puzzle className="w-3.5 h-3.5" /> Auto-generated
        </Badge>
      </div>

      {/* search + layout toggle */}
      <div className="flex items-center gap-3 flex-wrap">
        <div className="relative flex-1 min-w-[220px] max-w-md">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <Input
            placeholder="Search by title, id, or description..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-8"
          />
        </div>
        <div className="flex border rounded-md overflow-hidden">
          <button
            onClick={() => setLayout("grid")}
            className={`p-2 transition-colors ${
              layout === "grid"
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
            }`}
            aria-label="Grid layout"
          >
            <LayoutGrid className="w-4 h-4" />
          </button>
          <button
            onClick={() => setLayout("list")}
            className={`p-2 transition-colors ${
              layout === "list"
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
            }`}
            aria-label="List layout"
          >
            <LayoutList className="w-4 h-4" />
          </button>
        </div>
        <Label className="text-xs text-muted-foreground">{filtered.length} shown</Label>
      </div>

      {/* category tabs */}
      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <ScrollArea className="pb-2">
          <TabsList className="h-auto flex-wrap justify-start gap-1 bg-transparent p-0">
            {usedCategories.map((cat) => (
              <TabsTrigger
                key={cat}
                value={cat}
                className="capitalize px-3 py-1.5 text-xs rounded-md"
              >
                {cat}
              </TabsTrigger>
            ))}
          </TabsList>
        </ScrollArea>

        {/* gallery */}
        <div className="mt-4">
          <ScrollArea className="h-[calc(100vh-340px)]">
            {filtered.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-20 text-muted-foreground">
                <Eye className="w-10 h-10 mb-3 opacity-30" />
                <p className="text-sm">No schemas match your filters.</p>
              </div>
            ) : layout === "grid" ? (
              /* grid layout */
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                {filtered.map((card) => (
                  <Card
                    key={card.pluginId}
                    className="cursor-pointer hover:shadow-lg hover:border-primary/30 transition-all duration-150 group"
                    onClick={() => navigate(`/plugin/${card.pluginId}`)}
                  >
                    <CardHeader className="pb-2">
                      <div className="flex items-start justify-between gap-2">
                        <div className="flex items-center gap-2 min-w-0">
                          <Puzzle className="w-4 h-4 text-primary shrink-0" />
                          <CardTitle className="text-sm leading-tight truncate">
                            {card.title}
                          </CardTitle>
                        </div>
                        <Badge
                          className={`shrink-0 text-[10px] px-1.5 ${CATEGORY_COLORS[card.category] ?? ""}`}
                          variant="outline"
                        >
                          {card.category}
                        </Badge>
                      </div>
                    </CardHeader>
                    <CardContent className="space-y-2.5 pb-2">
                      {/* badges row */}
                      <div className="flex items-center gap-2 flex-wrap">
                        <Badge variant="secondary" className="text-[10px]">
                          {card.fieldCount} fields
                        </Badge>
                        {card.version && (
                          <Badge variant="outline" className="text-[10px]">
                            v{card.version}
                          </Badge>
                        )}
                      </div>

                      {/* description */}
                      <p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
                        {card.description}
                      </p>

                      {/* mini field previews */}
                      {card.previewFields.length > 0 && (
                        <div className="border-t pt-2 mt-1 space-y-0.5">
                          {card.previewFields.map((f, i) => (
                            <MiniFieldPreview key={i} field={f} />
                          ))}
                          {card.fieldCount > maxPreviewFields && (
                            <p className="text-[10px] text-muted-foreground italic pt-0.5">
                              +{card.fieldCount - maxPreviewFields} more fields
                            </p>
                          )}
                        </div>
                      )}
                    </CardContent>
                    <CardFooter className="pt-0">
                      <Button
                        variant="outline"
                        size="sm"
                        className="w-full group-hover:bg-primary group-hover:text-primary-foreground transition-colors"
                        onClick={(e) => {
                          e.stopPropagation();
                          navigate(`/plugin/${card.pluginId}`);
                        }}
                      >
                        Deploy
                      </Button>
                    </CardFooter>
                  </Card>
                ))}
              </div>
            ) : (
              /* list layout */
              <div className="space-y-2">
                {filtered.map((card) => (
                  <Card
                    key={card.pluginId}
                    className="cursor-pointer hover:shadow-md hover:border-primary/30 transition-all group"
                    onClick={() => navigate(`/plugin/${card.pluginId}`)}
                  >
                    <div className="flex items-center gap-4 p-4">
                      <Puzzle className="w-5 h-5 text-primary shrink-0" />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="font-semibold text-sm text-foreground truncate">
                            {card.title}
                          </span>
                          <Badge
                            className={`text-[10px] px-1.5 ${CATEGORY_COLORS[card.category] ?? ""}`}
                            variant="outline"
                          >
                            {card.category}
                          </Badge>
                          <Badge variant="secondary" className="text-[10px]">
                            {card.fieldCount} fields
                          </Badge>
                          {card.version && (
                            <Badge variant="outline" className="text-[10px]">
                              v{card.version}
                            </Badge>
                          )}
                        </div>
                        <p className="text-xs text-muted-foreground mt-0.5 line-clamp-1">
                          {card.description}
                        </p>
                      </div>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          navigate(`/plugin/${card.pluginId}`);
                        }}
                      >
                        Deploy
                      </Button>
                    </div>
                  </Card>
                ))}
              </div>
            )}
          </ScrollArea>
        </div>
      </Tabs>
    </div>
  );
}
