import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Progress } from "@/components/ui/progress";
import { Button } from "@/components/ui/button";
import { Radio, Shield, Cpu, Clock, AlertTriangle, Pause, Eye } from "lucide-react";
import { getOrchestrationQueue, getAntiHallucination, getOrchestrationResources, getOrchestrationExecutions } from "@/lib/api";

export default function OrchestrationPage() {
  const queue = useQuery({ queryKey: ["orchQueue"], queryFn: getOrchestrationQueue, refetchInterval: 2000 });
  const antiHall = useQuery({ queryKey: ["antiHall"], queryFn: getAntiHallucination, refetchInterval: 10000 });
  const resources = useQuery({ queryKey: ["orchResources"], queryFn: getOrchestrationResources, refetchInterval: 5000 });
  const executions = useQuery({ queryKey: ["orchExec"], queryFn: getOrchestrationExecutions, refetchInterval: 3000 });

  const queueData = Array.isArray(queue.data) ? queue.data : queue.data?.queue ?? [];
  const execData = Array.isArray(executions.data) ? executions.data : executions.data?.executions ?? [];
  const ah = antiHall.data ?? {};
  const res = resources.data ?? {};
  const catches = Array.isArray(ah.catches) ? ah.catches : [];

  return (
    <div className="space-y-6 animate-slide-in">
      <div><h1 className="text-2xl font-bold text-foreground">Orchestration</h1><p className="text-sm text-muted-foreground mt-1">Execution queue & resource control</p></div>

      {/* 4-Quadrant Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Top-Left: Execution Queue */}
        <Card className="border-border/50">
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium text-muted-foreground flex items-center gap-2"><Radio className="h-3.5 w-3.5 text-primary" />Live Queue</CardTitle></CardHeader>
          <CardContent className="p-0">
            <Table>
              <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">#</TableHead><TableHead className="text-xs">Type</TableHead><TableHead className="text-xs">Name</TableHead><TableHead className="text-xs">Priority</TableHead><TableHead className="text-xs">Status</TableHead></TableRow></TableHeader>
              <TableBody>
                {queueData.length > 0 ? queueData.slice(0, 10).map((q: any, i: number) => (
                  <TableRow key={q.id ?? i} className="border-border/30">
                    <TableCell className="text-xs font-mono">{i + 1}</TableCell>
                    <TableCell><Badge variant="outline" className="text-[10px]">{q.type}</Badge></TableCell>
                    <TableCell className="text-sm">{q.name}</TableCell>
                    <TableCell><Badge variant="outline" className={`text-[10px] ${q.priority === "high" ? "text-destructive" : q.priority === "medium" ? "text-warning" : "text-info"}`}>{q.priority}</Badge></TableCell>
                    <TableCell><Badge variant="outline" className={`text-[10px] ${q.status === "running" ? "text-success" : ""}`}>{q.status}</Badge></TableCell>
                  </TableRow>
                )) : <TableRow><TableCell colSpan={5} className="text-center text-sm text-muted-foreground py-6">{queue.isLoading ? "Loading..." : "Queue empty"}</TableCell></TableRow>}
              </TableBody>
            </Table>
          </CardContent>
        </Card>

        {/* Top-Right: Anti-Hallucination */}
        <Card className="border-border/50">
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium text-muted-foreground flex items-center gap-2"><Shield className="h-3.5 w-3.5 text-warning" />Anti-Hallucination Monitor</CardTitle></CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-3 gap-3">
              <div className="p-3 rounded-lg bg-secondary text-center"><p className="text-xs text-muted-foreground">Validations</p><p className="text-lg font-bold text-foreground">{ah.validations ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary text-center"><p className="text-xs text-muted-foreground">Caught</p><p className="text-lg font-bold text-destructive">{ah.caught ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary text-center"><p className="text-xs text-muted-foreground">Confidence</p><p className="text-lg font-bold text-success">{ah.confidence ?? "—"}%</p></div>
            </div>
            {catches.length > 0 && (
              <div className="space-y-1">
                <p className="text-xs text-muted-foreground font-medium">Recent Catches</p>
                {catches.slice(0, 3).map((c: any, i: number) => (
                  <div key={i} className="flex items-center gap-2 p-2 rounded bg-destructive/5 text-xs">
                    <AlertTriangle className="h-3 w-3 text-destructive shrink-0" />
                    <span className="text-foreground/80 truncate flex-1">{c.issue}</span>
                    <Badge variant="outline" className="text-[10px]">{c.action}</Badge>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        {/* Bottom-Left: Resources */}
        <Card className="border-border/50">
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium text-muted-foreground flex items-center gap-2"><Cpu className="h-3.5 w-3.5 text-info" />Resource Allocation</CardTitle></CardHeader>
          <CardContent className="space-y-4">
            {res.agents && Array.isArray(res.agents) ? (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground font-medium">Agent Utilization</p>
                {res.agents.map((a: any, i: number) => (
                  <div key={i} className="flex items-center gap-3">
                    <span className="text-xs text-foreground w-24 truncate">{a.name}</span>
                    <Progress value={a.utilization ?? 0} className="h-1.5 flex-1" />
                    <span className="text-xs text-muted-foreground w-10 text-right">{a.utilization ?? 0}%</span>
                  </div>
                ))}
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-3">
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Concurrency</p><p className="text-sm font-medium">{res.current_concurrency ?? "—"} / {res.max_concurrency ?? "—"}</p></div>
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Rate</p><p className="text-sm font-medium">{res.requests_per_min ?? "—"} req/min</p></div>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Bottom-Right: Quick Stats */}
        <Card className="border-border/50">
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium text-muted-foreground flex items-center gap-2"><Clock className="h-3.5 w-3.5 text-success" />Quick Stats</CardTitle></CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-3">
              <div className="p-3 rounded-lg bg-secondary text-center"><p className="text-xs text-muted-foreground">Executions Today</p><p className="text-lg font-bold text-foreground">{res.total_executions ?? execData.length ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary text-center"><p className="text-xs text-muted-foreground">Avg Wait</p><p className="text-lg font-bold text-foreground">{res.avg_wait ?? "—"}s</p></div>
              <div className="p-3 rounded-lg bg-secondary text-center"><p className="text-xs text-muted-foreground">Error Rate</p><p className="text-lg font-bold text-destructive">{res.error_rate ?? "—"}%</p></div>
              <div className="p-3 rounded-lg bg-secondary text-center"><p className="text-xs text-muted-foreground">Token Budget</p><div className="mt-1"><Progress value={res.token_budget_used ?? 0} className="h-1.5" /><p className="text-xs mt-1">{res.token_budget_used ?? 0}%</p></div></div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Execution Tracker */}
      <Card className="border-border/50">
        <CardHeader className="pb-2"><CardTitle className="text-sm font-medium text-muted-foreground">Execution Tracker</CardTitle></CardHeader>
        <CardContent className="p-0">
          <Table>
            <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">ID</TableHead><TableHead className="text-xs">Type</TableHead><TableHead className="text-xs">Name</TableHead><TableHead className="text-xs">Status</TableHead><TableHead className="text-xs">Duration</TableHead><TableHead className="text-xs">Agent</TableHead><TableHead className="text-xs text-right">Actions</TableHead></TableRow></TableHeader>
            <TableBody>
              {execData.length > 0 ? execData.slice(0, 20).map((e: any, i: number) => (
                <TableRow key={e.id ?? i} className="border-border/30">
                  <TableCell className="text-xs font-mono">{e.id?.slice(0, 8) ?? i}</TableCell>
                  <TableCell><Badge variant="outline" className="text-[10px]">{e.type}</Badge></TableCell>
                  <TableCell className="text-sm">{e.name}</TableCell>
                  <TableCell><Badge variant="outline" className={`text-[10px] ${e.status === "running" ? "text-success" : e.status === "failed" ? "text-destructive" : ""}`}>{e.status}</Badge></TableCell>
                  <TableCell className="text-xs text-muted-foreground">{e.duration ?? "—"}</TableCell>
                  <TableCell className="text-xs text-muted-foreground">{e.agent ?? "—"}</TableCell>
                  <TableCell className="text-right"><div className="flex items-center justify-end gap-1"><Button size="sm" variant="ghost" className="h-6 w-6 p-0"><Eye className="h-3 w-3" /></Button><Button size="sm" variant="ghost" className="h-6 w-6 p-0"><Pause className="h-3 w-3" /></Button></div></TableCell>
                </TableRow>
              )) : <TableRow><TableCell colSpan={7} className="text-center text-sm text-muted-foreground py-8">{executions.isLoading ? "Loading..." : "No executions"}</TableCell></TableRow>}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}
