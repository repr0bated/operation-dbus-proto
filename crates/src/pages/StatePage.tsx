import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { HoverCard, HoverCardTrigger, HoverCardContent } from "@/components/ui/hover-card";
import {
  Link2,
  ShieldCheck,
  CheckCircle2,
  XCircle,
  Clock,
  Hash,
  FileJson,
  HardDrive,
  RefreshCw,
  ChevronRight,
  Loader2,
  Copy,
} from "lucide-react";

// ── Mock blockchain data ──────────────────────────────────────

const MOCK_BLOCKS = [
  {
    index: 847,
    timestamp: "2026-02-17T14:32:01Z",
    prev_hash: "a1b2c3d4e5f6…",
    hash: "f7e8d9c0b1a2…3456",
    event_type: "dbus.schema.update",
    payload: { service: "org.freedesktop.NetworkManager", path: "/" },
    verified: true,
  },
  {
    index: 846,
    timestamp: "2026-02-17T14:28:44Z",
    prev_hash: "9f8e7d6c5b4a…",
    hash: "a1b2c3d4e5f6…7890",
    event_type: "container.create",
    payload: { name: "wg-exit-us-east", template: "alpine-wireguard" },
    verified: true,
  },
  {
    index: 845,
    timestamp: "2026-02-17T14:15:22Z",
    prev_hash: "3c4d5e6f7a8b…",
    hash: "9f8e7d6c5b4a…1234",
    event_type: "network.bridge.update",
    payload: { bridge: "ovs-br0", action: "port_add", port: "veth-wg1" },
    verified: true,
  },
  {
    index: 844,
    timestamp: "2026-02-17T13:58:10Z",
    prev_hash: "7b8c9d0e1f2a…",
    hash: "3c4d5e6f7a8b…5678",
    event_type: "agent.tool_call",
    payload: { agent: "network-agent", tool: "ovs_add_port", approved: true },
    verified: true,
  },
  {
    index: 843,
    timestamp: "2026-02-17T13:45:33Z",
    prev_hash: "1a2b3c4d5e6f…",
    hash: "7b8c9d0e1f2a…9012",
    event_type: "state.snapshot",
    payload: { subvol: "state_v847", size_mb: 12.4 },
    verified: true,
  },
  {
    index: 842,
    timestamp: "2026-02-17T13:30:05Z",
    prev_hash: "e5f6a7b8c9d0…",
    hash: "1a2b3c4d5e6f…3456",
    event_type: "container.destroy",
    payload: { name: "wg-exit-eu-old", reason: "rotation" },
    verified: false,
  },
  {
    index: 841,
    timestamp: "2026-02-17T13:12:58Z",
    prev_hash: "d4e5f6a7b8c9…",
    hash: "e5f6a7b8c9d0…7890",
    event_type: "workflow.approval",
    payload: { workflow: "rotate-exit-nodes", step: "destroy-old", user: "admin" },
    verified: true,
  },
];

const EVENT_COLORS: Record<string, string> = {
  "dbus.schema.update": "bg-accent/15 text-accent",
  "container.create": "bg-primary/15 text-primary",
  "container.destroy": "bg-destructive/15 text-destructive",
  "network.bridge.update": "bg-warning/15 text-warning",
  "agent.tool_call": "bg-info/15 text-info",
  "state.snapshot": "bg-muted text-muted-foreground",
  "workflow.approval": "bg-primary/15 text-primary",
};

// ── Mock state data ───────────────────────────────────────────

const MOCK_STATE_ENTRIES = [
  {
    key: "dbus/org_freedesktop_NetworkManager/_",
    type: "dbus_interface",
    size: "4.2 KB",
    updated: "2026-02-17T14:32:01Z",
    hash: "a3f8c1d2…5678",
    subvol: "state_v847",
  },
  {
    key: "dbus/org_freedesktop_systemd1/_",
    type: "dbus_interface",
    size: "18.7 KB",
    updated: "2026-02-17T12:10:44Z",
    hash: "b4e9d2f3…9012",
    subvol: "state_v845",
  },
  {
    key: "containers/wg-exit-us-east",
    type: "lxc_config",
    size: "1.1 KB",
    updated: "2026-02-17T14:28:44Z",
    hash: "c5f0e3a4…3456",
    subvol: "state_v846",
  },
  {
    key: "containers/wg-exit-eu-west",
    type: "lxc_config",
    size: "1.1 KB",
    updated: "2026-02-17T10:05:12Z",
    hash: "d6a1f4b5…7890",
    subvol: "state_v840",
  },
  {
    key: "network/ovs-br0",
    type: "ovs_bridge",
    size: "2.8 KB",
    updated: "2026-02-17T14:15:22Z",
    hash: "e7b2a5c6…1234",
    subvol: "state_v845",
  },
  {
    key: "wireguard/wg0",
    type: "wg_config",
    size: "0.6 KB",
    updated: "2026-02-17T09:30:00Z",
    hash: "f8c3b6d7…5678",
    subvol: "state_v838",
  },
];

const TYPE_BADGES: Record<string, string> = {
  dbus_interface: "bg-accent/15 text-accent",
  lxc_config: "bg-primary/15 text-primary",
  ovs_bridge: "bg-warning/15 text-warning",
  wg_config: "bg-info/15 text-info",
};

function timeAgo(iso: string) {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  return `${hrs}h ago`;
}

// ── Blockchain Tab ────────────────────────────────────────────

function BlockchainTab() {
  const [verifying, setVerifying] = useState<number | null>(null);

  const handleVerify = (idx: number) => {
    setVerifying(idx);
    setTimeout(() => setVerifying(null), 800);
  };

  return (
    <div className="space-y-3">
      {/* Chain stats */}
      <div className="grid grid-cols-4 gap-3">
        {[
          { label: "Chain Length", value: "847", icon: Link2 },
          { label: "Verified", value: "846/847", icon: ShieldCheck },
          { label: "Last Block", value: "2m ago", icon: Clock },
          { label: "State Subvol", value: "v847", icon: HardDrive },
        ].map((s) => (
          <Card key={s.label} className="p-3">
            <div className="flex items-center gap-2 text-muted-foreground mb-1">
              <s.icon className="h-3.5 w-3.5" />
              <span className="text-[10px] uppercase tracking-widest">{s.label}</span>
            </div>
            <p className="text-lg font-semibold font-mono text-foreground">{s.value}</p>
          </Card>
        ))}
      </div>

      {/* Block list */}
      <ScrollArea className="h-[calc(100vh-320px)]">
        <div className="space-y-2">
          {MOCK_BLOCKS.map((block) => (
            <Card key={block.index} className="p-0 overflow-hidden">
              <div className="flex items-stretch">
                {/* Index column */}
                <div className="flex flex-col items-center justify-center w-16 bg-muted/40 border-r border-border px-2 py-3">
                  <span className="text-[10px] text-muted-foreground">Block</span>
                  <span className="text-sm font-mono font-bold text-foreground">
                    #{block.index}
                  </span>
                </div>

                {/* Content */}
                <div className="flex-1 px-4 py-3 space-y-1.5">
                  <div className="flex items-center gap-2">
                    <Badge
                      className={`text-[10px] ${EVENT_COLORS[block.event_type] ?? "bg-muted text-muted-foreground"}`}
                    >
                      {block.event_type}
                    </Badge>
                    <span className="text-[10px] text-muted-foreground ml-auto">
                      {timeAgo(block.timestamp)}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 text-[10px] font-mono text-muted-foreground">
                    <span className="flex items-center gap-1">
                      <Hash className="h-2.5 w-2.5" />
                      {block.hash}
                    </span>
                    <span>← {block.prev_hash}</span>
                  </div>
                  <pre className="text-[10px] font-mono text-foreground/70 bg-muted/30 rounded px-2 py-1 overflow-x-auto">
                    {JSON.stringify(block.payload)}
                  </pre>
                </div>

                {/* Verify column */}
                <div className="flex items-center px-3 border-l border-border">
                  {verifying === block.index ? (
                    <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                  ) : block.verified ? (
                    <button
                      onClick={() => handleVerify(block.index)}
                      className="flex items-center gap-1 text-primary hover:text-primary/80 transition-colors"
                      title="Verified — click to re-verify"
                    >
                      <CheckCircle2 className="h-4 w-4" />
                    </button>
                  ) : (
                    <button
                      onClick={() => handleVerify(block.index)}
                      className="flex items-center gap-1 text-destructive hover:text-destructive/80 transition-colors"
                      title="Verification failed — click to retry"
                    >
                      <XCircle className="h-4 w-4" />
                    </button>
                  )}
                </div>
              </div>
            </Card>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}

// ── State Tab ─────────────────────────────────────────────────

function StateTab() {
  return (
    <div className="space-y-3">
      {/* Stats */}
      <div className="grid grid-cols-3 gap-3">
        {[
          { label: "State Entries", value: String(MOCK_STATE_ENTRIES.length) },
          { label: "BTRFS Subvol", value: "state_v847" },
          { label: "Total Size", value: "28.5 KB" },
        ].map((s) => (
          <Card key={s.label} className="p-3">
            <p className="text-[10px] uppercase tracking-widest text-muted-foreground mb-1">
              {s.label}
            </p>
            <p className="text-lg font-semibold font-mono text-foreground">{s.value}</p>
          </Card>
        ))}
      </div>

      {/* Entry list */}
      <ScrollArea className="h-[calc(100vh-320px)]">
        <div className="space-y-1">
          {MOCK_STATE_ENTRIES.map((e) => (
            <HoverCard key={e.key} openDelay={200} closeDelay={100}>
              <HoverCardTrigger asChild>
                <div className="w-full text-left rounded-lg border px-3 py-2.5 transition-colors cursor-default border-border hover:border-primary/40 hover:bg-primary/5">
                  <div className="flex items-center gap-2">
                    <FileJson className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                    <span className="text-xs font-mono text-foreground truncate">
                      {e.key}
                    </span>
                    <Badge
                      className={`text-[10px] ml-auto shrink-0 ${TYPE_BADGES[e.type] ?? "bg-muted text-muted-foreground"}`}
                    >
                      {e.type}
                    </Badge>
                  </div>
                  <div className="flex items-center gap-3 mt-1 text-[10px] text-muted-foreground ml-5">
                    <span>{e.size}</span>
                    <span>{timeAgo(e.updated)}</span>
                    <span className="font-mono">{e.hash}</span>
                  </div>
                </div>
              </HoverCardTrigger>
              <HoverCardContent side="top" align="center" className="w-96 p-5 space-y-3" style={{ fontFamily: "'JetBrains Mono', monospace" }}>
                <p className="text-sm font-semibold text-foreground truncate">{e.key}</p>
                {[
                  ["Type", e.type],
                  ["Size", e.size],
                  ["Subvolume", e.subvol],
                  ["Updated", timeAgo(e.updated)],
                  ["Hash", e.hash],
                ].map(([label, value]) => (
                  <div key={label} className="flex justify-between text-xs">
                    <span className="text-muted-foreground">{label}</span>
                    <span className="font-semibold text-foreground">{value}</span>
                  </div>
                ))}
                <div className="flex gap-2 pt-2">
                  <Button variant="outline" size="sm" className="text-xs h-8 flex-1">
                    <FileJson className="h-3.5 w-3.5 mr-1" /> JSON
                  </Button>
                  <Button variant="outline" size="sm" className="text-xs h-8 flex-1">
                    <Copy className="h-3.5 w-3.5 mr-1" /> Hash
                  </Button>
                  <Button variant="outline" size="sm" className="text-xs h-8 flex-1">
                    <RefreshCw className="h-3.5 w-3.5 mr-1" /> Re-scan
                  </Button>
                </div>
              </HoverCardContent>
            </HoverCard>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}

// ── Main Page ─────────────────────────────────────────────────

export default function StatePage() {
  return (
    <div className="flex flex-col h-full">
      <header className="px-6 py-4 border-b border-border bg-card/50">
        <h1 className="text-lg font-semibold tracking-tight text-foreground">
          State & Audit
        </h1>
        <p className="text-xs text-muted-foreground mt-0.5">
          Blockchain audit trail &amp; BTRFS state management
        </p>
      </header>

      <div className="flex-1 overflow-hidden px-6 py-4">
        <Tabs defaultValue="blockchain" className="h-full flex flex-col">
          <TabsList className="w-fit">
            <TabsTrigger value="blockchain" className="text-xs gap-1.5">
              <Link2 className="h-3.5 w-3.5" />
              Blockchain / Audit
            </TabsTrigger>
            <TabsTrigger value="state" className="text-xs gap-1.5">
              <HardDrive className="h-3.5 w-3.5" />
              State Management
            </TabsTrigger>
          </TabsList>
          <TabsContent value="blockchain" className="flex-1 overflow-auto mt-3">
            <BlockchainTab />
          </TabsContent>
          <TabsContent value="state" className="flex-1 overflow-auto mt-3">
            <StateTab />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
