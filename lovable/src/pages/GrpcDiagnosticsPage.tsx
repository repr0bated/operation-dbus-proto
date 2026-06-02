import { useState, useCallback, useRef, useEffect } from "react";
import { useEventStore } from "@/stores/event-store";
import {
  stateSync, eventChainService, ovsdbMirror, runtimeMirror,
  pluginService, componentRegistry, mcpService, grpcBaseUrl,
} from "@/grpc/client";
import type { PluginInfo } from "@/grpc/types/plugin-service";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { SchemaPanel } from "@/components/json/SchemaPanel";
import type { JsonSchema } from "@/types/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Activity, AlertTriangle, CheckCircle2, ChevronDown, Loader2,
  Play, RefreshCw, Wifi, WifiOff, XCircle, Zap,
} from "lucide-react";
import { cn } from "@/lib/utils";

/* ── Types ─────────────────────────────────────────────────────────────── */

type ProbeStatus = "idle" | "running" | "ok" | "error";

interface ProbeResult {
  name: string;
  service: string;
  method: string;
  status: ProbeStatus;
  latencyMs: number | null;
  error: string | null;
  response: unknown;
  timestamp: number | null;
}

interface StreamProbe {
  name: string;
  service: string;
  method: string;
  status: ProbeStatus;
  messagesReceived: number;
  firstMessageMs: number | null;
  error: string | null;
  abort: (() => void) | null;
}

const INITIAL_UNARY_PROBES: ProbeResult[] = [
  { name: "Get State", service: "StateSync", method: "GetState", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "List Plugins", service: "PluginService", method: "ListPlugins", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "Get Events", service: "EventChainService", method: "GetEvents", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "Verify Chain", service: "EventChainService", method: "VerifyChain", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "List DBs (OVS)", service: "OvsdbMirror", method: "ListDbs", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "Bridge State", service: "OvsdbMirror", method: "GetBridgeState", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "System Info", service: "RuntimeMirror", method: "GetSystemInfo", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "List Services", service: "RuntimeMirror", method: "ListServices", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "List Interfaces", service: "RuntimeMirror", method: "ListInterfaces", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "NUMA Topology", service: "RuntimeMirror", method: "GetNumaTopology", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "Discover Components", service: "ComponentRegistry", method: "Discover", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "MCP Health", service: "McpService", method: "Health", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
  { name: "MCP List Tools", service: "McpService", method: "ListTools", status: "idle", latencyMs: null, error: null, response: null, timestamp: null },
];

const INITIAL_STREAM_PROBES: StreamProbe[] = [
  { name: "StateSync.Subscribe", service: "StateSync", method: "Subscribe", status: "idle", messagesReceived: 0, firstMessageMs: null, error: null, abort: null },
  { name: "EventChain.SubscribeEvents", service: "EventChainService", method: "SubscribeEvents", status: "idle", messagesReceived: 0, firstMessageMs: null, error: null, abort: null },
  { name: "OvsdbMirror.Monitor", service: "OvsdbMirror", method: "Monitor", status: "idle", messagesReceived: 0, firstMessageMs: null, error: null, abort: null },
  { name: "RuntimeMirror.StreamMetrics", service: "RuntimeMirror", method: "StreamMetrics", status: "idle", messagesReceived: 0, firstMessageMs: null, error: null, abort: null },
  { name: "ComponentRegistry.Watch", service: "ComponentRegistry", method: "Watch", status: "idle", messagesReceived: 0, firstMessageMs: null, error: null, abort: null },
];

/* ── Helpers ────────────────────────────────────────────────────────────── */

const statusIcon = (s: ProbeStatus) => {
  switch (s) {
    case "idle": return <Wifi className="h-4 w-4 text-muted-foreground" />;
    case "running": return <Loader2 className="h-4 w-4 text-primary animate-spin" />;
    case "ok": return <CheckCircle2 className="h-4 w-4 text-emerald-500" />;
    case "error": return <XCircle className="h-4 w-4 text-destructive" />;
  }
};

const statusBadge = (s: ProbeStatus) => {
  const map: Record<ProbeStatus, string> = {
    idle: "secondary",
    running: "default",
    ok: "default",
    error: "destructive",
  };
  return (
    <Badge variant={map[s] as any} className={cn(s === "ok" && "bg-emerald-500/20 text-emerald-400 border-emerald-500/30")}>
      {s === "running" ? "Testing…" : s.toUpperCase()}
    </Badge>
  );
};

function parseMaybeJson(value: string): unknown {
  if (!value.trim()) return {};
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

function parseJsonArray(value: string): unknown[] {
  const parsed = parseMaybeJson(value);
  if (!Array.isArray(parsed)) {
    throw new Error("Method arguments must be a JSON array");
  }
  return parsed;
}

function isJsonSchema(value: unknown): value is JsonSchema {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function SchemaResultView({ data }: { data: unknown }) {
  const result = data as { dialect?: unknown; version?: unknown; schema?: unknown };
  const schema = result.schema;

  return (
    <div className="grid grid-cols-1 2xl:grid-cols-2 gap-4">
      {isJsonSchema(schema) ? (
        <SchemaPanel schema={schema} />
      ) : (
        <Card className="bg-card border-border">
          <CardContent className="p-4 text-sm text-muted-foreground">
            Schema response did not contain a renderable JSON object.
          </CardContent>
        </Card>
      )}
      <JsonRenderer data={data} />
    </div>
  );
}

/* ── Component ──────────────────────────────────────────────────────────── */

export default function GrpcDiagnosticsPage() {
  const { connected, lastError, eventCounts, latestState, latestStats } = useEventStore();
  const [unaryProbes, setUnaryProbes] = useState<ProbeResult[]>(INITIAL_UNARY_PROBES);
  const [streamProbes, setStreamProbes] = useState<StreamProbe[]>(INITIAL_STREAM_PROBES);
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [selectedPluginId, setSelectedPluginId] = useState("");
  const [schemaResult, setSchemaResult] = useState<unknown>(null);
  const [schemaError, setSchemaError] = useState("");
  const [dbusObjectPath, setDbusObjectPath] = useState("");
  const [dbusInterface, setDbusInterface] = useState("");
  const [propertyName, setPropertyName] = useState("");
  const [methodName, setMethodName] = useState("");
  const [methodArgs, setMethodArgs] = useState("[]");
  const [dbusResult, setDbusResult] = useState<unknown>(null);
  const [dbusError, setDbusError] = useState("");
  const [schemaLoading, setSchemaLoading] = useState(false);
  const [runningAll, setRunningAll] = useState(false);
  const streamAbortsRef = useRef<Map<string, () => void>>(new Map());

  // Cleanup stream probes on unmount
  useEffect(() => {
    return () => {
      streamAbortsRef.current.forEach((abort) => { try { abort(); } catch {} });
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    pluginService.listPlugins()
      .then((response) => {
        if (cancelled) return;
        const nextPlugins = response.plugins ?? [];
        setPlugins(nextPlugins);
        if (!selectedPluginId && nextPlugins[0]) {
          setSelectedPluginId(nextPlugins[0].id);
          setDbusObjectPath(nextPlugins[0].dbusPath);
          setDbusInterface(nextPlugins[0].interfaces[0] ?? "");
        }
      })
      .catch((err) => {
        if (!cancelled) setSchemaError((err as Error).message);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedPluginId]);

  const selectedPlugin = plugins.find((plugin) => plugin.id === selectedPluginId);

  const selectPlugin = useCallback((pluginId: string) => {
    const plugin = plugins.find((item) => item.id === pluginId);
    setSelectedPluginId(pluginId);
    setSchemaResult(null);
    setSchemaError("");
    setDbusResult(null);
    setDbusError("");
    if (plugin) {
      setDbusObjectPath(plugin.dbusPath);
      setDbusInterface(plugin.interfaces[0] ?? "");
    }
  }, [plugins]);

  const fetchSelectedSchema = useCallback(async () => {
    if (!selectedPluginId) return;
    setSchemaLoading(true);
    setSchemaError("");
    try {
      const response = await pluginService.getSchema({ pluginId: selectedPluginId, format: "json" });
      const parsed = parseMaybeJson(response.schemaJson);
      setSchemaResult({
        pluginId: selectedPluginId,
        dialect: response.dialect,
        version: response.version,
        schema: parsed,
      });
    } catch (err) {
      setSchemaError((err as Error).message);
    } finally {
      setSchemaLoading(false);
    }
  }, [selectedPluginId]);

  const runGetProperty = useCallback(async () => {
    if (!selectedPluginId) return;
    setDbusError("");
    setDbusResult(null);
    try {
      const response = await pluginService.getProperty({
        pluginId: selectedPluginId,
        objectPath: dbusObjectPath,
        interfaceName: dbusInterface,
        propertyName,
      });
      setDbusResult({ operation: "GetProperty", ...response });
    } catch (err) {
      setDbusError((err as Error).message);
    }
  }, [dbusInterface, dbusObjectPath, propertyName, selectedPluginId]);

  const runCallMethod = useCallback(async () => {
    if (!selectedPluginId) return;
    setDbusError("");
    setDbusResult(null);
    try {
      const args = parseJsonArray(methodArgs);
      const response = await pluginService.callMethod({
        pluginId: selectedPluginId,
        objectPath: dbusObjectPath,
        interfaceName: dbusInterface,
        methodName,
        arguments: args,
        actorId: "op-web.grpc-diagnostics",
        capabilityId: "exp.service.grpc-diagnostics.call-method@v1",
      });
      setDbusResult({ operation: "CallMethod", ...response });
    } catch (err) {
      setDbusError((err as Error).message);
    }
  }, [dbusInterface, dbusObjectPath, methodArgs, methodName, selectedPluginId]);

  /* ── Unary probe runner ──────────────────────────────────────────────── */

  const runUnaryProbe = useCallback(async (index: number) => {
    setUnaryProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "running", error: null, response: null } : p));
    const probe = INITIAL_UNARY_PROBES[index];
    const start = performance.now();
    try {
      let response: unknown;
      switch (`${probe.service}.${probe.method}`) {
        case "StateSync.GetState":
          response = await stateSync.getState({ pluginId: "", objectPath: "" });
          break;
        case "PluginService.ListPlugins":
          response = await pluginService.listPlugins();
          break;
        case "EventChainService.GetEvents":
          response = await eventChainService.getEvents({ limit: 5 });
          break;
        case "EventChainService.VerifyChain":
          response = await eventChainService.verifyChain({ fromEventId: 0, toEventId: 0 });
          break;
        case "OvsdbMirror.ListDbs":
          response = await ovsdbMirror.listDbs();
          break;
        case "OvsdbMirror.GetBridgeState":
          response = await ovsdbMirror.getBridgeState();
          break;
        case "RuntimeMirror.GetSystemInfo":
          response = await runtimeMirror.getSystemInfo();
          break;
        case "RuntimeMirror.ListServices":
          response = await runtimeMirror.listServices();
          break;
        case "RuntimeMirror.ListInterfaces":
          response = await runtimeMirror.listInterfaces();
          break;
        case "RuntimeMirror.GetNumaTopology":
          response = await runtimeMirror.getNumaTopology();
          break;
        case "ComponentRegistry.Discover":
          response = await componentRegistry.discover();
          break;
        case "McpService.Health":
          response = await mcpService.health();
          break;
        case "McpService.ListTools":
          response = await mcpService.listTools({ limit: 25, offset: 0 });
          break;
        default:
          throw new Error("Unknown probe");
      }
      const latency = Math.round(performance.now() - start);
      setUnaryProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "ok", latencyMs: latency, response, timestamp: Date.now() } : p));
    } catch (err) {
      const latency = Math.round(performance.now() - start);
      setUnaryProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "error", latencyMs: latency, error: (err as Error).message, timestamp: Date.now() } : p));
    }
  }, []);

  /* ── Stream probe runner ─────────────────────────────────────────────── */

  const runStreamProbe = useCallback((index: number) => {
    const probe = INITIAL_STREAM_PROBES[index];
    const key = probe.name;

    // Abort existing
    const existing = streamAbortsRef.current.get(key);
    if (existing) { try { existing(); } catch {} }

    setStreamProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "running", messagesReceived: 0, firstMessageMs: null, error: null } : p));

    const start = performance.now();
    let gotFirst = false;

    try {
      let result: { stream: ReadableStream<unknown>; abort: () => void };
      switch (key) {
        case "StateSync.Subscribe":
          result = stateSync.subscribe({ includeInitialState: true });
          break;
        case "EventChain.SubscribeEvents":
          result = eventChainService.subscribeEvents({ fromEventId: 0 });
          break;
        case "OvsdbMirror.Monitor":
          result = ovsdbMirror.monitor({ database: "Open_vSwitch", monitorRequestsJson: JSON.stringify({ Bridge: {} }) });
          break;
        case "RuntimeMirror.StreamMetrics":
          result = runtimeMirror.streamMetrics({ intervalSeconds: 5, categories: [] });
          break;
        case "ComponentRegistry.Watch":
          result = componentRegistry.watch({ componentTypes: [], includeExisting: true });
          break;
        default:
          throw new Error("Unknown stream probe");
      }

      streamAbortsRef.current.set(key, result.abort);

      const reader = result.stream.getReader();
      (async () => {
        try {
          while (true) {
            const { done } = await reader.read();
            if (done) {
              setStreamProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "ok" } : p));
              break;
            }
            if (!gotFirst) {
              gotFirst = true;
              const firstMs = Math.round(performance.now() - start);
              setStreamProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "ok", firstMessageMs: firstMs, messagesReceived: 1 } : p));
            } else {
              setStreamProbes((prev) => prev.map((p, i) => i === index ? { ...p, messagesReceived: p.messagesReceived + 1 } : p));
            }
          }
        } catch (err) {
          if ((err as Error).name !== "AbortError") {
            setStreamProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "error", error: (err as Error).message } : p));
          }
        }
      })();
    } catch (err) {
      setStreamProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "error", error: (err as Error).message } : p));
    }
  }, []);

  const stopStreamProbe = useCallback((index: number) => {
    const key = INITIAL_STREAM_PROBES[index].name;
    const abort = streamAbortsRef.current.get(key);
    if (abort) { try { abort(); } catch {} streamAbortsRef.current.delete(key); }
    setStreamProbes((prev) => prev.map((p, i) => i === index ? { ...p, status: "idle", abort: null } : p));
  }, []);

  /* ── Run all ─────────────────────────────────────────────────────────── */

  const runAllProbes = useCallback(async () => {
    setRunningAll(true);
    const promises = INITIAL_UNARY_PROBES.map((_, i) => runUnaryProbe(i));
    INITIAL_STREAM_PROBES.forEach((_, i) => runStreamProbe(i));
    await Promise.allSettled(promises);
    setRunningAll(false);
  }, [runUnaryProbe, runStreamProbe]);

  const stopAllStreams = useCallback(() => {
    streamAbortsRef.current.forEach((abort) => { try { abort(); } catch {} });
    streamAbortsRef.current.clear();
    setStreamProbes(INITIAL_STREAM_PROBES);
  }, []);

  /* ── Derived stats ───────────────────────────────────────────────────── */

  const stateKeys = Object.keys(latestState).length;
  const totalEvents = Object.values(eventCounts).reduce((a, b) => a + b, 0);
  const statsKeys = latestStats ? Object.keys(latestStats).length : 0;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">gRPC Diagnostics</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Probe every gRPC-Web endpoint to pinpoint connection issues
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant={connected ? "default" : "destructive"} className={cn("gap-1.5", connected && "bg-emerald-500/20 text-emerald-400 border-emerald-500/30")}>
            {connected ? <Wifi className="h-3 w-3" /> : <WifiOff className="h-3 w-3" />}
            {connected ? "Connected" : "Disconnected"}
          </Badge>
          <Button size="sm" variant="outline" onClick={runAllProbes} disabled={runningAll}>
            {runningAll ? <Loader2 className="h-4 w-4 mr-1.5 animate-spin" /> : <Zap className="h-4 w-4 mr-1.5" />}
            Run All Probes
          </Button>
          <Button size="sm" variant="ghost" onClick={stopAllStreams}>
            <XCircle className="h-4 w-4 mr-1.5" /> Stop Streams
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
        <Card className="bg-card border-border">
          <CardContent className="p-4">
            <p className="text-xs font-medium text-muted-foreground">gRPC-Web Base</p>
            <p className="mt-1 text-xs font-mono text-foreground break-all">{grpcBaseUrl}</p>
          </CardContent>
        </Card>
        <Card className="bg-card border-border">
          <CardContent className="p-4">
            <p className="text-xs font-medium text-muted-foreground">REST API Base</p>
            <p className="mt-1 text-xs font-mono text-foreground break-all">{import.meta.env.VITE_API_BASE_URL || "/api"}</p>
          </CardContent>
        </Card>
        <Card className="bg-card border-border">
          <CardContent className="p-4">
            <p className="text-xs font-medium text-muted-foreground">Schema Source</p>
            <p className="mt-1 text-xs font-mono text-foreground">PluginService.GetSchema</p>
          </CardContent>
        </Card>
      </div>

      {/* Live Store Summary */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        {[
          { label: "Event Store", value: connected ? "Online" : "Offline", sub: `${totalEvents} events received`, ok: connected },
          { label: "State Keys", value: stateKeys.toString(), sub: "projected into latestState", ok: stateKeys > 0 },
          { label: "Metric Keys", value: statsKeys.toString(), sub: "in latestStats", ok: statsKeys > 0 },
          { label: "Last Error", value: lastError ? "Error" : "None", sub: lastError ?? "No errors", ok: !lastError },
        ].map((kpi) => (
          <Card key={kpi.label} className="bg-card border-border">
            <CardContent className="p-4">
              <div className="flex items-center gap-2 mb-1">
                {kpi.ok ? <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" /> : <AlertTriangle className="h-3.5 w-3.5 text-destructive" />}
                <span className="text-xs font-medium text-muted-foreground">{kpi.label}</span>
              </div>
              <p className="text-lg font-bold text-foreground">{kpi.value}</p>
              <p className="text-[11px] text-muted-foreground truncate">{kpi.sub}</p>
            </CardContent>
          </Card>
        ))}
      </div>

      {lastError && (
        <Card className="border-destructive/50 bg-destructive/5">
          <CardContent className="p-4">
            <div className="flex items-start gap-2">
              <AlertTriangle className="h-4 w-4 text-destructive mt-0.5 shrink-0" />
              <div>
                <p className="text-sm font-medium text-destructive">Connection Error</p>
                <p className="text-xs text-muted-foreground mt-1 font-mono break-all">{lastError}</p>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      <Tabs defaultValue="unary">
        <TabsList>
          <TabsTrigger value="schema">D-Bus Schema</TabsTrigger>
          <TabsTrigger value="unary">Unary RPCs ({unaryProbes.length})</TabsTrigger>
          <TabsTrigger value="streams">Server Streams ({streamProbes.length})</TabsTrigger>
          <TabsTrigger value="store">Store Inspector</TabsTrigger>
        </TabsList>

        <TabsContent value="schema" className="space-y-4 mt-4">
          <div className="grid grid-cols-1 xl:grid-cols-[360px_1fr] gap-4">
            <Card className="bg-card border-border">
              <CardHeader className="pb-2">
                <CardTitle className="text-sm">Plugin Contract</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <label className="block space-y-1">
                  <span className="text-[11px] font-medium text-muted-foreground">Plugin</span>
                  <select
                    value={selectedPluginId}
                    onChange={(event) => selectPlugin(event.target.value)}
                    className="w-full h-9 rounded-md border border-input bg-card px-2 text-xs font-mono outline-none"
                  >
                    {plugins.length === 0 && <option value="">No plugins loaded</option>}
                    {plugins.map((plugin) => (
                      <option key={plugin.id} value={plugin.id}>{plugin.id}</option>
                    ))}
                  </select>
                </label>

                {selectedPlugin && (
                  <div className="rounded-md border border-border bg-muted/20 p-3 space-y-2">
                    <p className="text-sm font-medium text-foreground">{selectedPlugin.name}</p>
                    <p className="text-[11px] text-muted-foreground">{selectedPlugin.description || "No description"}</p>
                    <div className="space-y-1 text-[11px] font-mono text-muted-foreground">
                      <p>path: {selectedPlugin.dbusPath}</p>
                      <p>version: {selectedPlugin.version}</p>
                    </div>
                    <div className="flex flex-wrap gap-1">
                      {selectedPlugin.interfaces.map((iface) => (
                        <Badge key={iface} variant="secondary" className="font-mono text-[10px]">{iface}</Badge>
                      ))}
                    </div>
                  </div>
                )}

                <Button size="sm" className="w-full" onClick={fetchSelectedSchema} disabled={!selectedPluginId || schemaLoading}>
                  {schemaLoading ? <Loader2 className="h-4 w-4 mr-1.5 animate-spin" /> : <RefreshCw className="h-4 w-4 mr-1.5" />}
                  Load Rendered Schema
                </Button>

                {schemaError && (
                  <div className="rounded-md border border-destructive/30 bg-destructive/10 p-2 text-xs font-mono text-destructive break-all">
                    {schemaError}
                  </div>
                )}
              </CardContent>
            </Card>

            <div className="space-y-4 min-w-0">
              {schemaResult ? (
                <SchemaResultView data={schemaResult} />
              ) : (
                <Card className="bg-card border-border">
                  <CardContent className="p-4 text-sm text-muted-foreground">
                    Select a plugin and load its schema. The rendered schema is the contract for D-Bus methods, MCP tools, gRPC shapes, and UI fields.
                  </CardContent>
                </Card>
              )}
            </div>
          </div>

          <Card className="bg-card border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">D-Bus Call Surface</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                <label className="block space-y-1">
                  <span className="text-[11px] font-medium text-muted-foreground">Object Path</span>
                  <input value={dbusObjectPath} onChange={(event) => setDbusObjectPath(event.target.value)} className="w-full h-9 rounded-md border border-input bg-card px-2 text-xs font-mono outline-none" />
                </label>
                <label className="block space-y-1">
                  <span className="text-[11px] font-medium text-muted-foreground">Interface</span>
                  <input value={dbusInterface} onChange={(event) => setDbusInterface(event.target.value)} className="w-full h-9 rounded-md border border-input bg-card px-2 text-xs font-mono outline-none" />
                </label>
              </div>

              <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                <div className="rounded-md border border-border p-3 space-y-2">
                  <p className="text-xs font-medium text-foreground">GetProperty</p>
                  <div className="flex gap-2">
                    <input value={propertyName} onChange={(event) => setPropertyName(event.target.value)} placeholder="property name" className="min-w-0 flex-1 h-9 rounded-md border border-input bg-card px-2 text-xs font-mono outline-none" />
                    <Button size="sm" onClick={runGetProperty} disabled={!selectedPluginId || !propertyName}>Read</Button>
                  </div>
                </div>

                <div className="rounded-md border border-border p-3 space-y-2">
                  <p className="text-xs font-medium text-foreground">CallMethod</p>
                  <input value={methodName} onChange={(event) => setMethodName(event.target.value)} placeholder="method name" className="w-full h-9 rounded-md border border-input bg-card px-2 text-xs font-mono outline-none" />
                  <textarea value={methodArgs} onChange={(event) => setMethodArgs(event.target.value)} className="w-full min-h-20 rounded-md border border-input bg-card px-2 py-2 text-xs font-mono outline-none" />
                  <Button size="sm" onClick={runCallMethod} disabled={!selectedPluginId || !methodName}>Call</Button>
                </div>
              </div>

              {dbusError && (
                <div className="rounded-md border border-destructive/30 bg-destructive/10 p-2 text-xs font-mono text-destructive break-all">
                  {dbusError}
                </div>
              )}
              {dbusResult !== null && <JsonRenderer data={dbusResult} />}
            </CardContent>
          </Card>
        </TabsContent>

        {/* ── Unary Tab ──────────────────────────────────────────────── */}
        <TabsContent value="unary" className="space-y-2 mt-4">
          {unaryProbes.map((probe, i) => (
            <Collapsible key={probe.name}>
              <Card className="bg-card border-border">
                <div className="flex items-center justify-between p-3">
                  <div className="flex items-center gap-3 min-w-0">
                    {statusIcon(probe.status)}
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-foreground">{probe.name}</p>
                      <p className="text-[11px] text-muted-foreground font-mono">
                        {probe.service}/{probe.method}
                      </p>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {probe.latencyMs !== null && (
                      <span className="text-xs font-mono text-muted-foreground">{probe.latencyMs}ms</span>
                    )}
                    {statusBadge(probe.status)}
                    <Button size="sm" variant="ghost" onClick={() => runUnaryProbe(i)} disabled={probe.status === "running"}>
                      <Play className="h-3.5 w-3.5" />
                    </Button>
                    {(probe.response || probe.error) && (
                      <CollapsibleTrigger asChild>
                        <Button size="sm" variant="ghost"><ChevronDown className="h-3.5 w-3.5" /></Button>
                      </CollapsibleTrigger>
                    )}
                  </div>
                </div>
                <CollapsibleContent>
                  <div className="border-t border-border p-3">
                    {probe.error && (
                      <div className="mb-2 p-2 rounded bg-destructive/10 border border-destructive/20">
                        <p className="text-xs font-mono text-destructive break-all">{probe.error}</p>
                      </div>
                    )}
                    {probe.response && (
                      <ScrollArea className="max-h-60">
                        <pre className="text-[11px] font-mono text-muted-foreground whitespace-pre-wrap break-all">
                          {JSON.stringify(probe.response, null, 2)}
                        </pre>
                      </ScrollArea>
                    )}
                  </div>
                </CollapsibleContent>
              </Card>
            </Collapsible>
          ))}
        </TabsContent>

        {/* ── Streams Tab ────────────────────────────────────────────── */}
        <TabsContent value="streams" className="space-y-2 mt-4">
          {streamProbes.map((probe, i) => (
            <Card key={probe.name} className="bg-card border-border">
              <div className="flex items-center justify-between p-3">
                <div className="flex items-center gap-3 min-w-0">
                  {statusIcon(probe.status)}
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-foreground">{probe.name}</p>
                    <p className="text-[11px] text-muted-foreground font-mono">
                      {probe.service}/{probe.method}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-3">
                  {probe.firstMessageMs !== null && (
                    <span className="text-xs font-mono text-muted-foreground">
                      TTFM: {probe.firstMessageMs}ms
                    </span>
                  )}
                  {probe.messagesReceived > 0 && (
                    <span className="text-xs font-mono text-muted-foreground flex items-center gap-1">
                      <Activity className="h-3 w-3" /> {probe.messagesReceived} msgs
                    </span>
                  )}
                  {statusBadge(probe.status)}
                  {probe.status === "running" || probe.status === "ok" ? (
                    <Button size="sm" variant="ghost" onClick={() => stopStreamProbe(i)}>
                      <XCircle className="h-3.5 w-3.5" />
                    </Button>
                  ) : (
                    <Button size="sm" variant="ghost" onClick={() => runStreamProbe(i)}>
                      <Play className="h-3.5 w-3.5" />
                    </Button>
                  )}
                </div>
              </div>
              {probe.error && (
                <div className="border-t border-border p-3">
                  <div className="p-2 rounded bg-destructive/10 border border-destructive/20">
                    <p className="text-xs font-mono text-destructive break-all">{probe.error}</p>
                  </div>
                </div>
              )}
            </Card>
          ))}
        </TabsContent>

        {/* ── Store Inspector Tab ────────────────────────────────────── */}
        <TabsContent value="store" className="space-y-4 mt-4">
          <Card className="bg-card border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">Event Counts by Type</CardTitle>
            </CardHeader>
            <CardContent>
              {Object.keys(eventCounts).length === 0 ? (
                <p className="text-xs text-muted-foreground">No events received yet</p>
              ) : (
                <div className="flex flex-wrap gap-2">
                  {Object.entries(eventCounts).map(([type, count]) => (
                    <Badge key={type} variant="secondary" className="font-mono gap-1.5">
                      {type} <span className="text-primary">{count}</span>
                    </Badge>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <Card className="bg-card border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">Projected State Keys ({stateKeys})</CardTitle>
            </CardHeader>
            <CardContent>
              <ScrollArea className="max-h-64">
                {stateKeys === 0 ? (
                  <p className="text-xs text-muted-foreground">No state projected yet — streams may not be connected</p>
                ) : (
                  <div className="space-y-1">
                    {Object.entries(latestState).slice(0, 100).map(([key, val]) => (
                      <div key={key} className="flex items-start justify-between gap-4 py-1 border-b border-border/50 last:border-0">
                        <span className="text-[11px] font-mono text-foreground shrink-0">{key}</span>
                        <span className="text-[11px] font-mono text-muted-foreground truncate max-w-[50%] text-right">
                          {typeof val === "object" ? JSON.stringify(val).slice(0, 80) : String(val)}
                        </span>
                      </div>
                    ))}
                  </div>
                )}
              </ScrollArea>
            </CardContent>
          </Card>

          <Card className="bg-card border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">Latest Stats ({statsKeys} metrics)</CardTitle>
            </CardHeader>
            <CardContent>
              <ScrollArea className="max-h-64">
                {!latestStats || statsKeys === 0 ? (
                  <p className="text-xs text-muted-foreground">No metrics received — RuntimeMirror.StreamMetrics may not be connected</p>
                ) : (
                  <pre className="text-[11px] font-mono text-muted-foreground whitespace-pre-wrap">
                    {JSON.stringify(latestStats, null, 2)}
                  </pre>
                )}
              </ScrollArea>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
