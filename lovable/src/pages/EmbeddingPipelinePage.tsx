import { useState, useEffect } from "react";
import { Loader2, Activity, AlertTriangle, CheckCircle, Zap, Clock, BarChart3 } from "lucide-react";
import { cn } from "@/lib/utils";
import { embeddingService, blockchainService } from "@/grpc/client";
import type { EmbedRequestInfo, EmbeddingWorkerStatus, ChannelDiagnostics } from "@/grpc/types/embedding";
import type { QdrantCollectionRole } from "@/grpc/types/blockchain";

// ── Mock data ───────────────────────────────────────────────────────────────

const MOCK_WORKER: EmbeddingWorkerStatus = {
  active: true, currentRequest: "block-abc123", provider: "openclaw:embedder-voyage4lite",
  model: "voyage-4-lite", vectorDimension: 1024,
  retryConfig: { maxAttempts: 5, baseDelayMs: 500, backoffMultiplier: 2 },
  stats: { embedsCompleted: 2847, embedsFailed: 3, avgLatencyMs: 142, uptime: "4d 12h" },
};

const MOCK_CHANNEL: ChannelDiagnostics = {
  capacity: 1024, currentUsage: 7, dropPolicy: "try_send, silent drop on full",
  rationale: "embedding is runtime cognitive ability, not audit",
  auditSourceOfTruth: "BTRFS timing_subvol (never dropped)",
  totalDropped: 0, dropRate: 0,
};

const MOCK_QUEUE: EmbedRequestInfo[] = [
  { blockHash: "block-abc123", embeddingText: "plugin=firewall operation=set_rules actor=chatbot outcome=success summary=Applied isolation rules", collection: "op_footprints", payload: {}, status: "processing", attempt: 1, queuedAt: new Date(Date.now() - 5000).toISOString() },
  { blockHash: "block-def456", embeddingText: "plugin=dns operation=restart actor=chatbot outcome=success summary=Restarted NextDNS", collection: "op_footprints", payload: {}, status: "queued", attempt: 0, queuedAt: new Date(Date.now() - 2000).toISOString() },
];

const MOCK_ROLES: QdrantCollectionRole[] = [
  { name: "op_footprints", role: "ai_analysis", pointCount: 2847, diskSizeBytes: 1073741824, lastUpdated: new Date(Date.now() - 60000).toISOString() },
  { name: "ctl_plane_reasoning_episodes", role: "ai_analysis", pointCount: 384, diskSizeBytes: 268435456, lastUpdated: new Date(Date.now() - 300000).toISOString() },
  { name: "op_footprints_snapshot", role: "disaster_recovery", pointCount: 2800, diskSizeBytes: 1048576000, lastUpdated: new Date(Date.now() - 86400000).toISOString() },
];

function formatBytes(b: number): string {
  if (b < 1048576) return `${(b / 1024).toFixed(0)} KB`;
  if (b < 1073741824) return `${(b / 1048576).toFixed(1)} MB`;
  return `${(b / 1073741824).toFixed(1)} GB`;
}

const statusColors: Record<string, string> = {
  queued: "bg-muted text-muted-foreground",
  processing: "bg-primary/10 text-primary",
  completed: "bg-accent/50 text-accent-foreground",
  failed: "bg-destructive/10 text-destructive",
  dropped: "bg-destructive/10 text-destructive",
};

export default function EmbeddingPipelinePage() {
  const [worker, setWorker] = useState<EmbeddingWorkerStatus | null>(null);
  const [channel, setChannel] = useState<ChannelDiagnostics | null>(null);
  const [queue, setQueue] = useState<EmbedRequestInfo[]>([]);
  const [roles, setRoles] = useState<QdrantCollectionRole[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function load() {
      try {
        const [wRes, cRes, qRes, rRes] = await Promise.all([
          embeddingService.getWorkerStatus(),
          embeddingService.getChannelDiagnostics(),
          embeddingService.getQueue({ limit: 20 }),
          blockchainService.getQdrantRoles(),
        ]);
        setWorker(wRes.worker);
        setChannel(cRes.channel);
        setQueue(qRes.requests);
        setRoles(rRes.roles);
      } catch {
        setWorker(MOCK_WORKER);
        setChannel(MOCK_CHANNEL);
        setQueue(MOCK_QUEUE);
        setRoles(MOCK_ROLES);
      } finally {
        setLoading(false);
      }
    }
    load();
  }, []);

  if (loading) return <div className="flex items-center justify-center h-64"><Loader2 className="h-6 w-6 animate-spin text-muted-foreground" /></div>;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[26px] font-bold tracking-tight text-foreground">Embedding Pipeline</h1>
        <p className="text-sm text-muted-foreground mt-0.5">
          Footprint → mpsc channel → Embedding Worker → OpenClaw/Voyage → Qdrant
        </p>
      </div>

      {/* Worker + Channel side by side */}
      <div className="grid gap-4 md:grid-cols-2">
        {/* Worker Status */}
        {worker && (
          <div className="rounded-lg border border-border bg-card p-4 space-y-3">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold flex items-center gap-2">
                <Zap className="h-4 w-4 text-primary" /> Embedding Worker
              </h2>
              <span className={cn(
                "text-[10px] font-medium px-1.5 py-0.5 rounded",
                worker.active ? "bg-accent/50 text-accent-foreground" : "bg-destructive/10 text-destructive"
              )}>
                {worker.active ? "Active" : "Stopped"}
              </span>
            </div>
            <div className="grid grid-cols-2 gap-2 text-xs">
              <div><span className="text-muted-foreground">Provider:</span> <span className="font-mono">{worker.provider}</span></div>
              <div><span className="text-muted-foreground">Model:</span> <span className="font-mono">{worker.model}</span></div>
              <div><span className="text-muted-foreground">Dimensions:</span> {worker.vectorDimension}</div>
              <div><span className="text-muted-foreground">Uptime:</span> {worker.stats.uptime}</div>
            </div>
            <div className="grid grid-cols-3 gap-2">
              <div className="text-center p-2 bg-muted/30 rounded">
                <p className="text-lg font-bold text-foreground">{worker.stats.embedsCompleted.toLocaleString()}</p>
                <p className="text-[10px] text-muted-foreground">Completed</p>
              </div>
              <div className="text-center p-2 bg-muted/30 rounded">
                <p className="text-lg font-bold text-foreground">{worker.stats.embedsFailed}</p>
                <p className="text-[10px] text-muted-foreground">Failed</p>
              </div>
              <div className="text-center p-2 bg-muted/30 rounded">
                <p className="text-lg font-bold text-foreground">{worker.stats.avgLatencyMs}ms</p>
                <p className="text-[10px] text-muted-foreground">Avg Latency</p>
              </div>
            </div>
            <div className="text-[10px] text-muted-foreground">
              Retry: {worker.retryConfig.maxAttempts} attempts, {worker.retryConfig.baseDelayMs}ms base, exponential backoff
            </div>
          </div>
        )}

        {/* Channel Diagnostics */}
        {channel && (
          <div className="rounded-lg border border-border bg-card p-4 space-y-3">
            <h2 className="text-sm font-semibold flex items-center gap-2">
              <Activity className="h-4 w-4 text-primary" /> mpsc Channel
            </h2>
            <div className="space-y-2">
              <div className="flex justify-between text-xs">
                <span className="text-muted-foreground">Usage</span>
                <span>{channel.currentUsage} / {channel.capacity}</span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div className="h-full bg-primary rounded-full" style={{ width: `${(channel.currentUsage / channel.capacity) * 100}%` }} />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-2 text-xs">
              <div><span className="text-muted-foreground">Dropped:</span> <span className={channel.totalDropped > 0 ? "text-destructive font-bold" : ""}>{channel.totalDropped}</span></div>
              <div><span className="text-muted-foreground">Drop rate:</span> {channel.dropRate}/min</div>
            </div>
            <div className="bg-muted/30 rounded p-2 space-y-1">
              <p className="text-[10px] text-muted-foreground"><strong>Policy:</strong> {channel.dropPolicy}</p>
              <p className="text-[10px] text-muted-foreground"><strong>Rationale:</strong> {channel.rationale}</p>
              <p className="text-[10px] text-muted-foreground"><strong>Audit truth:</strong> {channel.auditSourceOfTruth}</p>
            </div>
          </div>
        )}
      </div>

      {/* Queue */}
      <div>
        <h2 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
          <Clock className="h-4 w-4 text-primary" /> Embed Queue
        </h2>
        <div className="space-y-2">
          {queue.length === 0 && <p className="text-sm text-muted-foreground">Queue empty — all requests processed.</p>}
          {queue.map((req) => (
            <div key={req.blockHash} className="rounded-lg border border-border bg-card p-3">
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs font-mono font-medium text-foreground">{req.blockHash}</span>
                <span className={cn("text-[10px] font-medium px-1.5 py-0.5 rounded", statusColors[req.status] ?? "bg-muted text-muted-foreground")}>
                  {req.status}{req.attempt > 0 ? ` (attempt ${req.attempt})` : ""}
                </span>
              </div>
              <p className="text-xs text-muted-foreground font-mono break-all mb-1">{req.embeddingText}</p>
              <div className="flex gap-3 text-[10px] text-muted-foreground">
                <span>Collection: {req.collection}</span>
                <span>Queued: {new Date(req.queuedAt).toLocaleTimeString()}</span>
                {req.error && <span className="text-destructive">{req.error}</span>}
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Qdrant Roles */}
      <div>
        <h2 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
          <BarChart3 className="h-4 w-4 text-primary" /> Qdrant Collection Roles
        </h2>
        <div className="grid gap-3 md:grid-cols-3">
          {roles.map((r) => (
            <div key={r.name} className="rounded-lg border border-border bg-card p-3 space-y-1">
              <div className="flex items-center justify-between">
                <span className="text-sm font-mono font-medium text-foreground">{r.name}</span>
                <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded">{r.role.replace("_", " ")}</span>
              </div>
              <div className="flex gap-3 text-[10px] text-muted-foreground">
                <span>{r.pointCount.toLocaleString()} points</span>
                <span>{formatBytes(r.diskSizeBytes)}</span>
                <span>Updated {new Date(r.lastUpdated).toLocaleTimeString()}</span>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
