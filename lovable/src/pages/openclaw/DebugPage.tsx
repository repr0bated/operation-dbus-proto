import { useState } from "react";
import { Activity, Cpu, Database, RefreshCw, Wifi, Server, Terminal } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

const statusData = {
  gateway: "running",
  version: "2026.2.26",
  uptime: "4d 12h 33m",
  pid: 42891,
  memory: "128 MB",
  sessions: 3,
  activeRuns: 1,
  wsConnections: 2,
};

const models = [
  { id: "claude-4-opus", provider: "anthropic", status: "available", latency: "1.2s" },
  { id: "claude-4-sonnet", provider: "anthropic", status: "available", latency: "0.8s" },
  { id: "gpt-5", provider: "openai", status: "available", latency: "1.5s" },
  { id: "gpt-5-mini", provider: "openai", status: "available", latency: "0.4s" },
  { id: "llama-4-405b", provider: "groq", status: "unavailable", latency: "—" },
];

const nodes = [
  { id: "gateway-main", type: "gateway", caps: ["chat", "cron", "skills", "exec"], status: "online" },
  { id: "worker-1", type: "node", caps: ["exec", "browser", "shell"], status: "online" },
];

const eventLog = [
  { time: "14:32:01", event: "chat.send", session: "default", detail: "runId=r_abc123" },
  { time: "14:32:03", event: "skill.call", session: "default", detail: "web-search → 3 results" },
  { time: "14:32:05", event: "chat.complete", session: "default", detail: "tokens: 1,247 in / 892 out" },
  { time: "14:30:00", event: "cron.run", session: "isolated", detail: "Check Server Health → success" },
  { time: "14:15:02", event: "session.create", session: "coding", detail: "model=claude-4-opus" },
  { time: "14:10:00", event: "cron.run", session: "isolated", detail: "Check Server Health → success" },
];

export default function DebugPage() {
  const [rpcMethod, setRpcMethod] = useState("status");
  const [rpcResult, setRpcResult] = useState("");

  const runRpc = () => {
    setRpcResult(JSON.stringify({ status: "ok", method: rpcMethod, ts: new Date().toISOString() }, null, 2));
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Debug</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Status, health, models, event log & manual RPC
          </p>
        </div>
        <Button variant="outline" size="sm" className="gap-1.5">
          <RefreshCw className="h-4 w-4" /> Refresh All
        </Button>
      </div>

      {/* Status cards */}
      <div className="grid gap-4 md:grid-cols-4">
        {[
          { label: "Status", value: statusData.gateway, icon: Activity, color: "text-green-500" },
          { label: "Version", value: statusData.version, icon: Server, color: "text-primary" },
          { label: "Uptime", value: statusData.uptime, icon: Cpu, color: "text-primary" },
          { label: "Memory", value: statusData.memory, icon: Database, color: "text-primary" },
        ].map((s) => (
          <Card key={s.label} className="bg-card border-border">
            <CardContent className="p-4 flex items-center gap-3">
              <s.icon className={`h-5 w-5 ${s.color}`} />
              <div>
                <p className="text-xs text-muted-foreground">{s.label}</p>
                <p className="text-sm font-semibold text-foreground">{s.value}</p>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Models */}
        <Card className="bg-card border-border">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2">
              <Cpu className="h-4 w-4 text-primary" /> Models
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {models.map((m) => (
                <div key={m.id} className="flex items-center justify-between py-1.5 text-sm">
                  <div className="flex items-center gap-2">
                    <span className={`h-1.5 w-1.5 rounded-full ${m.status === "available" ? "bg-green-500" : "bg-red-500"}`} />
                    <span className="font-mono text-foreground">{m.id}</span>
                    <Badge variant="outline" className="text-[10px] h-5">{m.provider}</Badge>
                  </div>
                  <span className="text-xs text-muted-foreground">{m.latency}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* Nodes */}
        <Card className="bg-card border-border">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2">
              <Wifi className="h-4 w-4 text-primary" /> Nodes
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {nodes.map((n) => (
                <div key={n.id} className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="h-1.5 w-1.5 rounded-full bg-green-500" />
                    <span className="font-semibold text-sm text-foreground">{n.id}</span>
                    <Badge variant="secondary" className="text-[10px] h-5">{n.type}</Badge>
                  </div>
                  <div className="flex gap-1 ml-4">
                    {n.caps.map((c) => (
                      <Badge key={c} variant="outline" className="text-[10px] h-5">{c}</Badge>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Event Log */}
      <Card className="bg-card border-border">
        <CardHeader className="pb-3">
          <CardTitle className="text-sm">Event Log</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-1">
            {eventLog.map((e, i) => (
              <div key={i} className="flex items-center gap-3 text-xs font-mono py-1 border-b border-border/50 last:border-0">
                <span className="text-muted-foreground w-16 shrink-0">{e.time}</span>
                <Badge variant="secondary" className="text-[10px] h-5 shrink-0">{e.event}</Badge>
                <span className="text-muted-foreground shrink-0">{e.session}</span>
                <span className="text-foreground truncate">{e.detail}</span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Manual RPC */}
      <Card className="bg-card border-border">
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Terminal className="h-4 w-4 text-primary" /> Manual RPC
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex gap-2">
            <Input
              value={rpcMethod}
              onChange={(e) => setRpcMethod(e.target.value)}
              placeholder="RPC method (e.g. status, health, models.list)"
              className="font-mono text-sm"
            />
            <Button onClick={runRpc} size="sm">Call</Button>
          </div>
          {rpcResult && (
            <pre className="bg-muted rounded-md p-3 text-xs font-mono text-foreground overflow-auto max-h-40">
              {rpcResult}
            </pre>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
