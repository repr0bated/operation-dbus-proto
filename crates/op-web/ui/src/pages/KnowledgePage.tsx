/**
 * Knowledge Store & ML Manager — manages the local Qdrant knowledge base,
 * Local ML Grinder status, model downloads, and GPU heartbeat.
 * All data derived from useEventStore.latestState — no mock data.
 */
import { useMemo, useState } from "react";
import { useEventStore } from "@/stores/event-store";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { Card, Pill, StatusDot } from "@/components/shell/Primitives";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import {
  Brain, ChevronRight, Cloud, Cpu, Database, Download,
  HardDrive, Search, Server, Thermometer,
} from "lucide-react";
import { cn } from "@/lib/utils";

/* ── Grinder status badge ── */
const GRINDER_STATUS: Record<string, { variant: "ok" | "warn" | "default"; label: string }> = {
  grinding: { variant: "ok", label: "Grinding" },
  idle: { variant: "default", label: "Idle" },
  paused: { variant: "warn", label: "Paused" },
};

export default function KnowledgePage() {
  const latestState = useEventStore((s) => s.latestState);
  const [searchQuery, setSearchQuery] = useState("");
  const [useCloud, setUseCloud] = useState(false);

  /* ── Derive knowledge state ── */
  const knowledge = useMemo(() => {
    const raw =
      latestState["knowledge"] ??
      latestState["knowledge.store"] ??
      latestState["qdrant:knowledge"] ??
      {};
    return raw as Record<string, unknown>;
  }, [latestState]);

  const grinder = useMemo(() => {
    const raw =
      latestState["ml.grinder"] ??
      latestState["grinder"] ??
      latestState["op-ml:grinder"] ??
      {};
    return raw as Record<string, unknown>;
  }, [latestState]);

  const models = useMemo(() => {
    const raw =
      latestState["ml.models"] ??
      latestState["models.local"] ??
      latestState["op-ml:models"] ??
      [];
    return Array.isArray(raw) ? (raw as Record<string, unknown>[]) : [];
  }, [latestState]);

  const gpu = useMemo(() => {
    const raw =
      latestState["gpu"] ??
      latestState["gpu.heartbeat"] ??
      latestState["paperspace:gpu"] ??
      null;
    return raw as Record<string, unknown> | null;
  }, [latestState]);

  const searchResults = useMemo(() => {
    const raw =
      latestState["knowledge.search_results"] ??
      latestState["qdrant:search_results"] ??
      [];
    return Array.isArray(raw) ? (raw as Record<string, unknown>[]) : [];
  }, [latestState]);

  /* ── Derived metrics ── */
  const pointsIndexed = Number(knowledge.points_indexed ?? grinder.points_indexed ?? 0);
  const pointsTarget = Number(knowledge.points_target ?? grinder.points_target ?? 250_000);
  const progressPct = pointsTarget > 0 ? Math.min((pointsIndexed / pointsTarget) * 100, 100) : 0;
  const grinderStatus = String(grinder.status ?? "idle").toLowerCase();
  const grinderMeta = GRINDER_STATUS[grinderStatus] ?? GRINDER_STATUS.idle;

  const gpuStatus = gpu ? String((gpu as any).status ?? "unknown") : null;
  const gpuTemp = gpu ? Number((gpu as any).temperature ?? 0) : 0;
  const gpuUtil = gpu ? Number((gpu as any).utilization ?? 0) : 0;

  const hasData = Object.keys(knowledge).length > 0 || models.length > 0 || Object.keys(grinder).length > 0;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
            <Brain className="h-6 w-6 text-primary" />
            Knowledge &amp; ML
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Qdrant knowledge base, local ML grinder, and model management
          </p>
        </div>

        {/* GPU Heartbeat */}
        {gpu && (
          <Card className="flex items-center gap-3 px-4 py-2">
            <Server className="h-4 w-4 text-muted-foreground" />
            <div className="text-xs">
              <div className="font-medium">P5000 GPU</div>
              <div className="flex items-center gap-2 text-muted-foreground">
                <StatusDot status={gpuStatus === "running" ? "ok" : gpuStatus === "idle" ? "warn" : "offline"} />
                <span className="capitalize">{gpuStatus}</span>
                {gpuTemp > 0 && (
                  <span className="flex items-center gap-0.5">
                    <Thermometer className="h-3 w-3" /> {gpuTemp}°C
                  </span>
                )}
                {gpuUtil > 0 && <span>{gpuUtil}%</span>}
              </div>
            </div>
          </Card>
        )}
      </div>

      {!hasData && (
        <Card className="p-8 text-center text-muted-foreground">
          <Database className="h-8 w-8 mx-auto mb-2 opacity-40" />
          <p className="text-sm">Waiting for live data from knowledge store and ML grinder…</p>
        </Card>
      )}

      {/* ── Indexing Progress ── */}
      <Card className="p-5 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold flex items-center gap-2">
            <HardDrive className="h-4 w-4 text-primary" />
            Local Grinder — Embedding Progress
          </h2>
          <Pill variant={grinderMeta.variant}>
            <StatusDot status={grinderMeta.variant === "ok" ? "ok" : grinderMeta.variant === "warn" ? "warn" : "offline"} />
            {grinderMeta.label}
          </Pill>
        </div>
        <Progress value={progressPct} className="h-3" />
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span className="font-mono">
            {pointsIndexed.toLocaleString()} / {pointsTarget.toLocaleString()} Points Indexed
          </span>
          <span className="font-mono">{progressPct.toFixed(1)}%</span>
        </div>
      </Card>

      {/* ── Cost / Provider Toggle ── */}
      <Card className="p-5">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold flex items-center gap-2">
            {useCloud ? <Cloud className="h-4 w-4 text-primary" /> : <Cpu className="h-4 w-4 text-primary" />}
            Embedding Provider
          </h2>
          <div className="flex items-center gap-3">
            <span className={cn("text-xs font-medium", !useCloud && "text-foreground")}>Local</span>
            <Switch checked={useCloud} onCheckedChange={setUseCloud} />
            <span className={cn("text-xs font-medium", useCloud && "text-foreground")}>Cloud</span>
          </div>
        </div>
        <div className="mt-3">
          {useCloud ? (
            <Badge variant="outline" className="border-warn/40 text-warn bg-warn/10">
              ⚠ Cloud usage (Vertex AI) may incur costs
            </Badge>
          ) : (
            <Badge variant="outline" className="border-ok/40 text-ok bg-ok/10">
              ✅ Cost-Free Local Mode (CPU via op-ml)
            </Badge>
          )}
        </div>
      </Card>

      <Tabs defaultValue="search" className="w-full">
        <TabsList>
          <TabsTrigger value="search">
            <Search className="h-3.5 w-3.5 mr-1.5" /> Vector Search
          </TabsTrigger>
          <TabsTrigger value="models">
            <Download className="h-3.5 w-3.5 mr-1.5" /> Model Downloader
          </TabsTrigger>
          <TabsTrigger value="details">
            <Database className="h-3.5 w-3.5 mr-1.5" /> Store Details
          </TabsTrigger>
        </TabsList>

        {/* ── Vector Search Playground ── */}
        <TabsContent value="search" className="space-y-4">
          <Card className="p-4">
            <div className="flex gap-2">
              <Input
                placeholder="Semantic search across 3tched_knowledge…"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="flex-1"
              />
              <Button size="sm">
                <Search className="h-4 w-4 mr-1" /> Search
              </Button>
            </div>
          </Card>

          {searchResults.length === 0 ? (
            <Card className="p-6 text-center text-sm text-muted-foreground">
              {searchQuery
                ? "No results — waiting for backend response…"
                : "Enter a query to search the 3tched_knowledge collection."}
            </Card>
          ) : (
            <div className="space-y-2">
              <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider px-1">
                Semantic Matches ({searchResults.length})
              </h3>
              {searchResults.map((result, i) => (
                <Collapsible key={i}>
                  <Card className="overflow-hidden">
                    <CollapsibleTrigger className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-muted/30 transition-colors">
                      <ChevronRight className="h-4 w-4 text-muted-foreground shrink-0 transition-transform [[data-state=open]>&]:rotate-90" />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <Badge variant="secondary" className="text-[10px]">
                            {String(result.repo ?? result.source ?? "unknown")}
                          </Badge>
                          {result.score != null && (
                            <span className="text-[10px] font-mono text-muted-foreground">
                              score: {Number(result.score).toFixed(3)}
                            </span>
                          )}
                        </div>
                        <p className="text-xs text-muted-foreground mt-1 truncate">
                          {String(result.snippet ?? result.text ?? result.content ?? "")}
                        </p>
                      </div>
                    </CollapsibleTrigger>
                    <CollapsibleContent>
                      <div className="border-t border-border p-3">
                        <JsonRenderer data={result} />
                      </div>
                    </CollapsibleContent>
                  </Card>
                </Collapsible>
              ))}
            </div>
          )}
        </TabsContent>

        {/* ── Model Downloader ── */}
        <TabsContent value="models" className="space-y-4">
          {models.length === 0 ? (
            <Card className="p-6 text-center text-sm text-muted-foreground">
              <Download className="h-6 w-6 mx-auto mb-2 opacity-40" />
              Waiting for model list from op-ml…
            </Card>
          ) : (
            <div className="space-y-2">
              {models.map((model, i) => {
                const name = String(model.name ?? model.id ?? `model-${i}`);
                const status = String(model.status ?? "available");
                const dlProgress = Number(model.download_progress ?? model.progress ?? 0);
                const size = model.size ? String(model.size) : null;
                const isDownloading = status === "downloading";
                const isReady = status === "ready" || status === "loaded";

                return (
                  <Card key={i} className="p-4">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <Brain className="h-4 w-4 text-primary" />
                        <div>
                          <span className="text-sm font-medium font-mono">{name}</span>
                          {size && (
                            <span className="text-[10px] text-muted-foreground ml-2">{size}</span>
                          )}
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <Pill variant={isReady ? "ok" : isDownloading ? "warn" : "default"}>
                          <StatusDot status={isReady ? "ok" : isDownloading ? "warn" : "offline"} />
                          <span className="capitalize">{status}</span>
                        </Pill>
                        {!isReady && !isDownloading && (
                          <Button variant="outline" size="sm">
                            <Download className="h-3.5 w-3.5 mr-1" /> Download
                          </Button>
                        )}
                      </div>
                    </div>
                    {isDownloading && (
                      <div className="mt-3 space-y-1">
                        <Progress value={dlProgress} className="h-2" />
                        <span className="text-[10px] font-mono text-muted-foreground">
                          {dlProgress.toFixed(1)}%
                        </span>
                      </div>
                    )}
                    {Object.keys(model).length > 3 && (
                      <Collapsible>
                        <CollapsibleTrigger className="mt-2 text-[11px] text-muted-foreground hover:text-foreground flex items-center gap-1">
                          <ChevronRight className="h-3 w-3 transition-transform [[data-state=open]>&]:rotate-90" />
                          Details
                        </CollapsibleTrigger>
                        <CollapsibleContent>
                          <div className="mt-2 border-t border-border pt-2">
                            <JsonRenderer data={model} />
                          </div>
                        </CollapsibleContent>
                      </Collapsible>
                    )}
                  </Card>
                );
              })}
            </div>
          )}
        </TabsContent>

        {/* ── Store Details ── */}
        <TabsContent value="details" className="space-y-4">
          {Object.keys(knowledge).length === 0 ? (
            <Card className="p-6 text-center text-sm text-muted-foreground">
              Waiting for knowledge store metadata…
            </Card>
          ) : (
            <Card className="p-4">
              <JsonRenderer data={knowledge} />
            </Card>
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}
