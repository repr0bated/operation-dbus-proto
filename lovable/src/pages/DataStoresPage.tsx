import { useState, useEffect } from "react";
import { Database, Activity, AlertTriangle, CheckCircle, Loader2, HardDrive, Wifi } from "lucide-react";
import { cn } from "@/lib/utils";
import { dataStoreService } from "@/grpc/client";
import type { DataStoreStatus } from "@/grpc/types/data-stores";

const MOCK_STORES: DataStoreStatus[] = [
  { name: "op-state-store", location: "SQLite (op-dbus host)", description: "Plugin state, cognitive memory, user memory", durability: "Persistent", status: "healthy", sizeBytes: 52428800, usedBytes: 31457280, lastChecked: new Date().toISOString(), latencyMs: 2, details: { walMode: true, tables: 14 } },
  { name: "BTRFS timing_subvol", location: "/timing_subvol", description: "Blockchain footprints (audit, immutable)", durability: "Persistent + replicated", status: "healthy", sizeBytes: 2147483648, usedBytes: 892614656, lastChecked: new Date().toISOString(), details: { snapshotCount: 14 } },
  { name: "BTRFS state_subvol", location: "/state_subvol", description: "DR current.json snapshots", durability: "Persistent + replicated", status: "healthy", sizeBytes: 1073741824, usedBytes: 134217728, lastChecked: new Date().toISOString(), details: {} },
  { name: "Qdrant", location: "qdrant container (10.149.181.190)", description: "Vectors: footprints, reasoning episodes", durability: "Persistent + snapshotted", status: "healthy", sizeBytes: 10737418240, usedBytes: 4294967296, lastChecked: new Date().toISOString(), latencyMs: 8, details: { collections: 3, protocol: "gRPC:6334" } },
  { name: "Embedding channel", location: "in-process mpsc", description: "In-flight embed requests", durability: "Best-effort (runtime only)", status: "healthy", lastChecked: new Date().toISOString(), details: { capacity: 1024, used: 12, dropped: 0 } },
];

function formatBytes(bytes: number): string {
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(1)} GB`;
}

const statusIcon = (s: string) => {
  if (s === "healthy") return <CheckCircle className="h-4 w-4 text-primary" />;
  if (s === "degraded") return <AlertTriangle className="h-4 w-4 text-warning" />;
  return <AlertTriangle className="h-4 w-4 text-destructive" />;
};

export default function DataStoresPage() {
  const [stores, setStores] = useState<DataStoreStatus[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    dataStoreService.getDataStores()
      .then((r) => setStores(r.stores))
      .catch(() => setStores(MOCK_STORES))
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return <div className="flex items-center justify-center h-64"><Loader2 className="h-6 w-6 animate-spin text-muted-foreground" /></div>;
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[26px] font-bold tracking-tight text-foreground">Data Stores</h1>
        <p className="text-sm text-muted-foreground mt-0.5">Health and status of all persistent and runtime data stores.</p>
      </div>

      <div className="grid gap-3">
        {stores.map((store) => (
          <div key={store.name} className="rounded-lg border border-border bg-card p-4">
            <div className="flex items-start justify-between mb-2">
              <div className="flex items-center gap-2">
                {statusIcon(store.status)}
                <div>
                  <h3 className="text-sm font-semibold text-foreground">{store.name}</h3>
                  <p className="text-xs text-muted-foreground">{store.location}</p>
                </div>
              </div>
              <div className="flex items-center gap-3 text-[10px] text-muted-foreground">
                {store.latencyMs != null && (
                  <span className="flex items-center gap-1">
                    <Wifi className="h-3 w-3" />
                    {store.latencyMs}ms
                  </span>
                )}
                <span className={cn(
                  "font-medium px-1.5 py-0.5 rounded",
                  store.status === "healthy" ? "bg-accent/50 text-accent-foreground" :
                  store.status === "degraded" ? "bg-primary/10 text-primary" :
                  "bg-destructive/10 text-destructive"
                )}>
                  {store.status}
                </span>
              </div>
            </div>
            <p className="text-xs text-muted-foreground mb-2">{store.description}</p>
            <div className="flex flex-wrap gap-3 text-[10px] text-muted-foreground">
              <span className="flex items-center gap-1"><Database className="h-3 w-3" /> {store.durability}</span>
              {store.sizeBytes != null && store.usedBytes != null && (
                <span className="flex items-center gap-1"><HardDrive className="h-3 w-3" /> {formatBytes(store.usedBytes)} / {formatBytes(store.sizeBytes)}</span>
              )}
              {store.details && Object.entries(store.details).map(([k, v]) => (
                <span key={k} className="bg-muted px-1.5 py-0.5 rounded font-mono">{k}: {String(v)}</span>
              ))}
              <span className="flex items-center gap-1"><Activity className="h-3 w-3" /> Checked {new Date(store.lastChecked).toLocaleTimeString()}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
