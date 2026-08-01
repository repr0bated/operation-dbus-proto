import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Bot, LayoutGrid, List, RefreshCw, Settings, Activity, BarChart3, Wrench } from "lucide-react";
import { getAgents, getAgentDetail, getAgentHistory, getAgentMetrics, updateAgentConfig, restartAgent } from "@/lib/api";

const statusColors: Record<string, string> = {
  active: "text-success border-success/20 bg-success/10",
  idle: "text-warning border-warning/20 bg-warning/10",
  error: "text-destructive border-destructive/20 bg-destructive/10",
};

function AgentDetailDialog({ agentId, open, onClose }: { agentId: string | null; open: boolean; onClose: () => void }) {
  const queryClient = useQueryClient();
  const detail = useQuery({ queryKey: ["agentDetail", agentId], queryFn: () => getAgentDetail(agentId!), enabled: !!agentId });
  const history = useQuery({ queryKey: ["agentHistory", agentId], queryFn: () => getAgentHistory(agentId!), enabled: !!agentId });
  const metrics = useQuery({ queryKey: ["agentMetrics", agentId], queryFn: () => getAgentMetrics(agentId!), enabled: !!agentId });
  const restartMut = useMutation({ mutationFn: () => restartAgent(agentId!), onSuccess: () => queryClient.invalidateQueries({ queryKey: ["agents"] }) });

  const agent = detail.data ?? {};
  const histData = Array.isArray(history.data) ? history.data : history.data?.history ?? [];

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {agent.name ?? "Agent"}
            {agent.status && <Badge variant="outline" className={`text-xs ${statusColors[agent.status] ?? ""}`}>{agent.status}</Badge>}
          </DialogTitle>
        </DialogHeader>
        <div className="flex gap-2 mb-3">
          <Button size="sm" variant="outline" className="text-xs gap-1.5" onClick={() => restartMut.mutate()} disabled={restartMut.isPending}>
            <RefreshCw className="h-3 w-3" />{restartMut.isPending ? "Restarting..." : "Restart"}
          </Button>
        </div>
        <Tabs defaultValue="overview">
          <TabsList className="bg-secondary mb-3">
            <TabsTrigger value="overview" className="text-xs">Overview</TabsTrigger>
            <TabsTrigger value="config" className="text-xs">Config</TabsTrigger>
            <TabsTrigger value="history" className="text-xs">History</TabsTrigger>
            <TabsTrigger value="metrics" className="text-xs">Metrics</TabsTrigger>
          </TabsList>
          <TabsContent value="overview" className="space-y-3">
            <p className="text-sm text-muted-foreground">{agent.description ?? "No description"}</p>
            <div className="grid grid-cols-2 gap-3">
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Type</p><p className="text-sm font-medium">{agent.type ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Tools</p><p className="text-sm font-medium">{agent.tools_count ?? agent.tools ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Active Tasks</p><p className="text-sm font-medium">{agent.active_tasks ?? 0}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Uptime</p><p className="text-sm font-medium">{agent.uptime ?? "—"}</p></div>
            </div>
          </TabsContent>
          <TabsContent value="config" className="space-y-3">
            <div><p className="text-xs text-muted-foreground mb-1">System Prompt</p><Textarea className="font-mono text-xs bg-secondary border-none min-h-[80px]" defaultValue={agent.system_prompt ?? ""} /></div>
            <div><p className="text-xs text-muted-foreground mb-1">Model</p>
              <Select defaultValue={agent.model ?? "gpt-4"}><SelectTrigger className="h-8 bg-secondary border-none text-sm"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="gpt-4">GPT-4</SelectItem><SelectItem value="claude-3">Claude 3</SelectItem><SelectItem value="gemini-pro">Gemini Pro</SelectItem></SelectContent></Select>
            </div>
            <div><p className="text-xs text-muted-foreground mb-1">Temperature: {agent.temperature ?? 0.7}</p><Slider defaultValue={[agent.temperature ?? 0.7]} min={0} max={1} step={0.1} /></div>
          </TabsContent>
          <TabsContent value="history">
            <Table>
              <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">Time</TableHead><TableHead className="text-xs">Task</TableHead><TableHead className="text-xs">Duration</TableHead><TableHead className="text-xs">Status</TableHead></TableRow></TableHeader>
              <TableBody>
                {histData.length > 0 ? histData.slice(0, 20).map((h: any, i: number) => (
                  <TableRow key={i} className="border-border/30"><TableCell className="text-xs font-mono">{h.timestamp}</TableCell><TableCell className="text-sm">{h.task}</TableCell><TableCell className="text-xs">{h.duration}</TableCell><TableCell><Badge variant="outline" className={`text-[10px] ${h.status === "success" ? "text-success" : "text-destructive"}`}>{h.status}</Badge></TableCell></TableRow>
                )) : <TableRow><TableCell colSpan={4} className="text-center text-sm text-muted-foreground py-6">{history.isLoading ? "Loading..." : "No history"}</TableCell></TableRow>}
              </TableBody>
            </Table>
          </TabsContent>
          <TabsContent value="metrics" className="space-y-3">
            {metrics.data ? (
              <div className="grid grid-cols-2 gap-3">
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Tasks/Hour</p><p className="text-lg font-bold">{metrics.data.tasks_per_hour ?? "—"}</p></div>
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Success Rate</p><p className="text-lg font-bold">{metrics.data.success_rate ?? "—"}%</p></div>
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Avg Duration</p><p className="text-lg font-bold">{metrics.data.avg_duration ?? "—"}ms</p></div>
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Tokens Used</p><p className="text-lg font-bold">{metrics.data.tokens_used ?? "—"}</p></div>
              </div>
            ) : <p className="text-sm text-muted-foreground">{metrics.isLoading ? "Loading..." : "No metrics"}</p>}
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

export default function AgentsPage() {
  const [view, setView] = useState<"table" | "cards">("cards");
  const [filter, setFilter] = useState("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const agents = useQuery({ queryKey: ["agents"], queryFn: getAgents, refetchInterval: 10000 });
  const data = Array.isArray(agents.data) ? agents.data : agents.data?.agents ?? [];
  const filtered = data.filter((a: any) => filter === "all" || a.status === filter || a.type?.toLowerCase() === filter);

  return (
    <div className="space-y-6 animate-slide-in">
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div><h1 className="text-2xl font-bold text-foreground">Agents</h1><p className="text-sm text-muted-foreground mt-1">AI agent management & monitoring</p></div>
        <div className="flex items-center gap-2">
          <Select value={filter} onValueChange={setFilter}><SelectTrigger className="h-8 w-28 bg-secondary border-none text-xs"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="all">All</SelectItem><SelectItem value="active">Active</SelectItem><SelectItem value="idle">Idle</SelectItem><SelectItem value="error">Error</SelectItem><SelectItem value="cognitive">Cognitive</SelectItem><SelectItem value="mcp">MCP</SelectItem></SelectContent></Select>
          <div className="flex border border-border rounded-md">
            <Button size="sm" variant={view === "cards" ? "secondary" : "ghost"} className="h-8 px-2" onClick={() => setView("cards")}><LayoutGrid className="h-3.5 w-3.5" /></Button>
            <Button size="sm" variant={view === "table" ? "secondary" : "ghost"} className="h-8 px-2" onClick={() => setView("table")}><List className="h-3.5 w-3.5" /></Button>
          </div>
        </div>
      </div>

      {view === "cards" ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filtered.length > 0 ? filtered.map((a: any) => (
            <Card key={a.id ?? a.name} className="border-border/50 card-glow cursor-pointer hover:border-primary/30 transition-colors" onClick={() => setSelectedId(a.id)}>
              <CardContent className="p-4">
                <div className="flex items-start justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <div className={`p-2 rounded-lg ${a.status === "active" ? "bg-success/10 text-success" : a.status === "error" ? "bg-destructive/10 text-destructive" : "bg-muted text-muted-foreground"}`}>
                      <Bot className="h-4 w-4" />
                    </div>
                    <div><h3 className="text-sm font-semibold text-foreground">{a.name}</h3><p className="text-[10px] text-muted-foreground">{a.type ?? "agent"}</p></div>
                  </div>
                  <Badge variant="outline" className={`text-[10px] ${statusColors[a.status] ?? ""}`}>{a.status}</Badge>
                </div>
                <div className="flex items-center gap-4 mt-3 text-xs text-muted-foreground">
                  <span className="flex items-center gap-1"><Wrench className="h-3 w-3" />{a.tools_count ?? a.tools ?? 0} tools</span>
                  <span className="flex items-center gap-1"><Activity className="h-3 w-3" />{a.active_tasks ?? 0} tasks</span>
                </div>
              </CardContent>
            </Card>
          )) : <div className="col-span-full text-center py-12 text-sm text-muted-foreground">{agents.isLoading ? "Loading..." : "No agents"}</div>}
        </div>
      ) : (
        <Card className="border-border/50"><CardContent className="p-0">
          <Table>
            <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">Name</TableHead><TableHead className="text-xs">Type</TableHead><TableHead className="text-xs">Status</TableHead><TableHead className="text-xs">Tools</TableHead><TableHead className="text-xs">Tasks</TableHead><TableHead className="text-xs">Uptime</TableHead></TableRow></TableHeader>
            <TableBody>
              {filtered.length > 0 ? filtered.map((a: any) => (
                <TableRow key={a.id ?? a.name} className="border-border/30 cursor-pointer hover:bg-accent/50" onClick={() => setSelectedId(a.id)}>
                  <TableCell className="text-sm font-medium">{a.name}</TableCell><TableCell><Badge variant="outline" className="text-[10px]">{a.type ?? "agent"}</Badge></TableCell>
                  <TableCell><Badge variant="outline" className={`text-[10px] ${statusColors[a.status] ?? ""}`}>{a.status}</Badge></TableCell>
                  <TableCell className="text-sm text-muted-foreground">{a.tools_count ?? 0}</TableCell><TableCell className="text-sm text-muted-foreground">{a.active_tasks ?? 0}</TableCell><TableCell className="text-xs text-muted-foreground">{a.uptime ?? "—"}</TableCell>
                </TableRow>
              )) : <TableRow><TableCell colSpan={6} className="text-center py-8 text-sm text-muted-foreground">{agents.isLoading ? "Loading..." : "No agents"}</TableCell></TableRow>}
            </TableBody>
          </Table>
        </CardContent></Card>
      )}

      <AgentDetailDialog agentId={selectedId} open={!!selectedId} onClose={() => setSelectedId(null)} />
    </div>
  );
}
