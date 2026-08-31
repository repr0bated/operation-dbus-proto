import { useCallback, useEffect, useState, useRef } from "react";
import { PageHeader, Card, Callout } from "@/components/shell/Primitives";
import { ErrorBanner } from "@/components/shell/Feedback";
import { resolveApiBase } from "@/lib/api";
import { Send, RotateCcw, Zap, Database, Search, Loader2 } from "lucide-react";

interface GenerationConfig {
  target_count: number;
  enable_mcp: boolean;
  enable_qdrant: boolean;
  operator_guidance: string;
}

interface GenerationStatus {
  running: boolean;
  generated: number;
  attempts: number;
  target: number;
  current_spec?: SpecPreview;
}

interface SpecPreview {
  id: string;
  preview: string;
}

interface LogEntry {
  timestamp: string;
  level: "info" | "warn" | "error";
  message: string;
}

export default function GalleryGenPage() {
  const [config, setConfig] = useState<GenerationConfig>({
    target_count: 10,
    enable_mcp: false,
    enable_qdrant: false,
    operator_guidance: "",
  });
  
  const [status, setStatus] = useState<GenerationStatus>({
    running: false,
    generated: 0,
    attempts: 0,
    target: 10,
  });
  
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  
  const eventSourceRef = useRef<EventSource | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);
  
  const scrollToBottom = useCallback(() => {
    logsEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);
  
  useEffect(() => {
    scrollToBottom();
  }, [logs, scrollToBottom]);
  
  const addLog = useCallback((level: LogEntry["level"], message: string) => {
    const timestamp = new Date().toISOString().split("T")[1].split(".")[0];
    setLogs((prev) => [...prev.slice(-199), { timestamp, level, message }]);
  }, []);
  
  const startGeneration = useCallback(async () => {
    setError(null);
    setLogs([]);
    setStatus({ running: true, generated: 0, attempts: 0, target: config.target_count });
    
    try {
      // Start generation via HTTP POST
      const response = await fetch(`${resolveApiBase()}/gallery-gen/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(config),
      });
      
      if (!response.ok) {
        throw new Error(`Failed to start generation: ${response.status}`);
      }
      
      // Connect to SSE stream for progress updates
      const eventSource = new EventSource(`${resolveApiBase()}/gallery-gen/stream`);
      eventSourceRef.current = eventSource;
      setConnected(true);
      
      eventSource.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          
          switch (data.type) {
            case "status":
              setStatus((prev) => ({
                ...prev,
                generated: data.generated ?? prev.generated,
                attempts: data.attempts ?? prev.attempts,
                current_spec: data.current_spec,
              }));
              break;
              
            case "log":
              addLog(data.level ?? "info", data.message);
              break;
              
            case "complete":
              addLog("info", `Generation complete: ${data.generated} specs in ${data.attempts} attempts`);
              setStatus((prev) => ({ ...prev, running: false }));
              eventSource.close();
              setConnected(false);
              break;
              
            case "error":
              addLog("error", data.message);
              setError(data.message);
              setStatus((prev) => ({ ...prev, running: false }));
              eventSource.close();
              setConnected(false);
              break;
          }
        } catch (e) {
          console.error("Failed to parse SSE data:", e);
        }
      };
      
      eventSource.onerror = () => {
        addLog("error", "Connection to generation stream lost");
        setConnected(false);
        setStatus((prev) => ({ ...prev, running: false }));
        eventSource.close();
      };
      
    } catch (e) {
      setError((e as Error).message);
      setStatus((prev) => ({ ...prev, running: false }));
      setConnected(false);
    }
  }, [config, addLog]);
  
  const stopGeneration = useCallback(() => {
    eventSourceRef.current?.close();
    setConnected(false);
    
    fetch(`${resolveApiBase()}/gallery-gen/stop`, { method: "POST" })
      .catch((e) => addLog("warn", `Failed to stop generation: ${(e as Error).message}`));
    
    setStatus((prev) => ({ ...prev, running: false }));
    addLog("info", "Generation stopped by operator");
  }, [addLog]);
  
  useEffect(() => {
    return () => {
      eventSourceRef.current?.close();
    };
  }, []);
  
  const progressPercent = status.target > 0 ? Math.min(100, (status.generated / status.target) * 100) : 0;
  
  return (
    <div className="space-y-6">
      <PageHeader
        title="Gallery Generation"
        subtitle="Model-agnostic UI spec generation with operator guidance"
      />
      
      {error && <ErrorBanner title="Generation Error" message={error} />}
      
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Configuration Panel */}
        <Card title="Configuration" className="lg:col-span-1">
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium mb-1.5">
                Target Count
              </label>
              <input
                type="number"
                value={config.target_count}
                onChange={(e) => setConfig((prev) => ({
                  ...prev,
                  target_count: parseInt(e.target.value) || 10,
                }))}
                disabled={status.running}
                className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm disabled:opacity-50"
                min={1}
                max={200}
              />
              <p className="text-xs text-muted-foreground mt-1">
                Number of specs to generate (1-200)
              </p>
            </div>
            
            <div className="space-y-2">
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={config.enable_mcp}
                  onChange={(e) => setConfig((prev) => ({
                    ...prev,
                    enable_mcp: e.target.checked,
                  }))}
                  disabled={status.running}
                  className="rounded border-border"
                />
                <span className="text-sm flex items-center gap-1.5">
                  <Database className="h-4 w-4" />
                  MCP Cross-Blob Discovery
                </span>
              </label>
              <p className="text-xs text-muted-foreground ml-6">
                Enable search_methods, search_subids, find_related tools
              </p>
            </div>
            
            <div className="space-y-2">
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={config.enable_qdrant}
                  onChange={(e) => setConfig((prev) => ({
                    ...prev,
                    enable_qdrant: e.target.checked,
                  }))}
                  disabled={status.running}
                  className="rounded border-border"
                />
                <span className="text-sm flex items-center gap-1.5">
                  <Search className="h-4 w-4" />
                  Qdrant Semantic Search
                </span>
              </label>
              <p className="text-xs text-muted-foreground ml-6">
                Enable semantic_search tool for vector similarity
              </p>
            </div>
          </div>
        </Card>
        
        {/* Operator Guidance Panel */}
        <Card title="Operator Guidance" className="lg:col-span-2">
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium mb-1.5">
                Guidance Prompt
              </label>
              <textarea
                value={config.operator_guidance}
                onChange={(e) => setConfig((prev) => ({
                  ...prev,
                  operator_guidance: e.target.value,
                }))}
                disabled={status.running}
                rows={4}
                className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono disabled:opacity-50"
                placeholder="Optional: Provide specific guidance for generation..."
              />
            </div>
            
            <Callout variant="default" className="text-xs">
              <strong>Universal Prompt:</strong> "Make this dataset as accessible to as many people, industries, causes as possible."
              This prompt is always included. Your guidance above will be added as additional context.
            </Callout>
            
            <div className="flex gap-2">
              <button
                onClick={status.running ? stopGeneration : startGeneration}
                className={`flex items-center gap-1.5 rounded-md px-4 py-2 text-sm font-medium ${
                  status.running
                    ? "bg-destructive text-destructive-foreground hover:bg-destructive/90"
                    : "bg-primary text-primary-foreground hover:bg-primary/90"
                }`}
              >
                {status.running ? (
                  <>
                    <RotateCcw className="h-4 w-4" />
                    Stop
                  </>
                ) : (
                  <>
                    <Zap className="h-4 w-4" />
                    Start Generation
                  </>
                )}
              </button>
            </div>
          </div>
        </Card>
      </div>
      
      {/* Progress Panel */}
      {status.running && (
        <Card title="Progress">
          <div className="space-y-3">
            <div className="flex items-center justify-between text-sm">
              <span>
                Generated: {status.generated} / {status.target}
              </span>
              <span className="text-muted-foreground">
                Attempts: {status.attempts}
              </span>
            </div>
            
            <div className="h-2 rounded-full bg-muted overflow-hidden">
              <div
                className="h-full bg-primary transition-all duration-300"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
            
            {status.current_spec && (
              <div className="rounded-md border border-border bg-muted/20 p-3">
                <p className="text-xs font-mono text-muted-foreground">Current spec:</p>
                <p className="text-sm font-mono truncate">{status.current_spec.preview}</p>
              </div>
            )}
            
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              {connected ? "Connected to generation stream" : "Connecting..."}
            </div>
          </div>
        </Card>
      )}
      
      {/* Logs Panel */}
      <Card title="Generation Logs">
        <div className="rounded-md border border-border bg-background/40 p-3 h-[400px] overflow-auto font-mono text-xs">
          {logs.length === 0 ? (
            <p className="text-muted-foreground">No logs yet. Start a generation session to see progress.</p>
          ) : (
            logs.map((log, i) => (
              <div
                key={i}
                className={`flex gap-2 py-0.5 ${
                  log.level === "error"
                    ? "text-destructive"
                    : log.level === "warn"
                    ? "text-amber-500"
                    : "text-foreground"
                }`}
              >
                <span className="text-muted-foreground">[{log.timestamp}]</span>
                <span className="uppercase text-[10px] font-bold opacity-70">
                  {log.level}
                </span>
                <span>{log.message}</span>
              </div>
            ))
          )}
          <div ref={logsEndRef} />
        </div>
      </Card>
    </div>
  );
}
