import { useState, useMemo } from "react";
import { PageHeader, Card, Pill, StatusDot } from "@/components/shell/Primitives";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { useEventStore } from "@/stores/event-store";
import { Search, ChevronRight, ShieldCheck, Lock, Link2, Hash } from "lucide-react";
import { cn } from "@/lib/utils";

interface AuditBlock {
  id: string;
  hash: string;
  prev_hash: string;
  timestamp: string;
  event_type: string;
  agent?: string;
  summary: string;
  payload: Record<string, unknown>;
}

const EVENT_COLORS: Record<string, string> = {
  "dbus.schema.update": "bg-blue-500/20 text-blue-400 border-blue-500/30",
  "agent.tool_call": "bg-purple-500/20 text-purple-400 border-purple-500/30",
  "agent.thought": "bg-violet-500/20 text-violet-400 border-violet-500/30",
  "network.bridge.update": "bg-cyan-500/20 text-cyan-400 border-cyan-500/30",
  "security.auth": "bg-green-500/20 text-green-400 border-green-500/30",
  "state.mutation": "bg-amber-500/20 text-amber-400 border-amber-500/30",
  "config.change": "bg-orange-500/20 text-orange-400 border-orange-500/30",
};

export default function SecurityPage() {
  const { latestState } = useEventStore();
  const [searchQuery, setSearchQuery] = useState("");
  const [expandedBlocks, setExpandedBlocks] = useState<Set<string>>(new Set());

  const liveStatus = useMemo(() => {
    const s = latestState["security.status"] ?? latestState["security:status"];
    const defaults = { auth_mode: "—", tls_status: "unknown", total_blocks: 0, qdrant_status: "unknown", vectors_indexed: 0 };
    if (s && typeof s === "object") return { ...defaults, ...(s as Record<string, unknown>) };
    return defaults;
  }, [latestState]);

  const blocks = useMemo(() => {
    const live = latestState["security.audit_chain"] ?? latestState["audit:chain"];
    const base = Array.isArray(live) ? (live as AuditBlock[]) : [];
    if (!searchQuery.trim()) return base;
    const q = searchQuery.toLowerCase();
    return base.filter((b) =>
      b.summary.toLowerCase().includes(q) ||
      b.event_type.toLowerCase().includes(q) ||
      (b.agent?.toLowerCase().includes(q)) ||
      JSON.stringify(b.payload).toLowerCase().includes(q)
    );
  }, [latestState, searchQuery]);

  const toggleBlock = (id: string) => {
    setExpandedBlocks((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  };

  return (
    <>
      <PageHeader title="Security & Audit" subtitle="Cryptographic audit trail with semantic search." />

      {/* Status Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-6">
        <Card title="Auth Mode" subtitle="Active authentication">
          <div className="flex items-center gap-2 mt-2">
            <ShieldCheck className="h-4 w-4 text-primary" />
            <span className="text-sm font-semibold text-foreground">{String(liveStatus.auth_mode)}</span>
          </div>
        </Card>
        <Card title="TLS Status" subtitle="Transport security">
          <div className="flex items-center gap-2 mt-2">
            <Lock className="h-4 w-4 text-primary" />
            <StatusDot status={liveStatus.tls_status === "active" ? "ok" : "error"} />
            <span className="text-sm font-semibold text-foreground">{String(liveStatus.tls_status)}</span>
          </div>
        </Card>
        <Card title="Audit Chain" subtitle="Total blocks">
          <div className="flex items-center gap-2 mt-2">
            <Hash className="h-4 w-4 text-primary" />
            <span className="text-lg font-mono font-bold text-foreground">{String(liveStatus.total_blocks)}</span>
          </div>
        </Card>
        <Card title="Qdrant" subtitle="Vector index">
          <div className="flex items-center gap-2 mt-2">
            <Link2 className="h-4 w-4 text-primary" />
            <StatusDot status={liveStatus.qdrant_status === "connected" ? "ok" : "error"} />
            <span className="text-sm text-muted-foreground">{String(liveStatus.vectors_indexed)} vectors</span>
          </div>
        </Card>
      </div>

      {/* Semantic Search */}
      <div className="relative mb-6">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Semantic Audit Search — search the system's reasoning, mutations, and decisions..."
          className="pl-10 font-mono text-sm bg-card border-border"
        />
      </div>

      {/* Blockchain Ledger */}
      <div className="space-y-1.5">
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-sm font-semibold text-foreground">Blockchain Ledger</h3>
          <span className="text-xs text-muted-foreground">{blocks.length} blocks</span>
        </div>
        {blocks.map((block) => {
          const isOpen = expandedBlocks.has(block.id);
          const colorClass = EVENT_COLORS[block.event_type] ?? "bg-muted text-muted-foreground border-border";
          return (
            <Collapsible key={block.id} open={isOpen} onOpenChange={() => toggleBlock(block.id)}>
              <CollapsibleTrigger className="w-full flex items-center gap-3 rounded-md border border-border bg-card px-4 py-2.5 hover:bg-muted/20 transition-colors text-left">
                <ChevronRight className={cn("h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform", isOpen && "rotate-90")} />
                <span className="font-mono text-[11px] text-muted-foreground shrink-0 w-24 truncate" title={block.hash}>{block.hash}</span>
                <Badge variant="outline" className={cn("text-[10px] font-mono shrink-0 border", colorClass)}>{block.event_type}</Badge>
                {block.agent && <Badge variant="secondary" className="text-[10px] shrink-0">{block.agent}</Badge>}
                <span className="text-xs text-foreground flex-1 truncate">{block.summary}</span>
                <span className="text-[10px] text-muted-foreground font-mono shrink-0">{new Date(block.timestamp).toLocaleTimeString()}</span>
              </CollapsibleTrigger>
              <CollapsibleContent>
                <div className="ml-8 mr-2 mt-1 mb-2 rounded-md border border-border overflow-hidden">
                  <JsonRenderer data={block.payload} defaultMode="tree" />
                </div>
              </CollapsibleContent>
            </Collapsible>
          );
        })}
      </div>
    </>
  );
}
