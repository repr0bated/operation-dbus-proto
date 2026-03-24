import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Layers, Play, Pause, SkipForward, X, LayoutGrid, List, Plus, Bot } from "lucide-react";
import { getWorkStacks, getWorkStackDetail, getWorkStackHistory, controlWorkStack } from "@/lib/api";

const priorityColors: Record<string, string> = { high: "text-destructive border-destructive/20", medium: "text-warning border-warning/20", low: "text-info border-info/20" };
const statusColumns = ["queued", "in_progress", "paused", "completed"] as const;
const statusLabels: Record<string, string> = { queued: "Queued", in_progress: "In Progress", paused: "Paused", completed: "Completed" };
const statusColColors: Record<string, string> = { queued: "bg-muted", in_progress: "bg-primary/10", paused: "bg-warning/10", completed: "bg-success/10" };

function StackDetailDialog({ stackId, open, onClose }: { stackId: string | null; open: boolean; onClose: () => void }) {
  const queryClient = useQueryClient();
  const detail = useQuery({ queryKey: ["stackDetail", stackId], queryFn: () => getWorkStackDetail(stackId!), enabled: !!stackId });
  const history = useQuery({ queryKey: ["stackHistory", stackId], queryFn: () => getWorkStackHistory(stackId!), enabled: !!stackId });
  const controlMut = useMutation({ mutationFn: (action: string) => controlWorkStack(stackId!, action), onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["workStacks"] }); queryClient.invalidateQueries({ queryKey: ["stackDetail", stackId] }); } });

  const stack = detail.data ?? {};
  const tasks = Array.isArray(stack.tasks) ? stack.tasks : [];
  const histData = Array.isArray(history.data) ? history.data : history.data?.history ?? [];

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-auto">
        <DialogHeader><DialogTitle className="flex items-center gap-2">{stack.name ?? "Stack"}{stack.priority && <Badge variant="outline" className={`text-xs ${priorityColors[stack.priority] ?? ""}`}>{stack.priority}</Badge>}</DialogTitle></DialogHeader>
        <div className="flex gap-2 mb-3">
          <Button size="sm" variant="outline" className="text-xs gap-1" onClick={() => controlMut.mutate("start")}><Play className="h-3 w-3" />Start</Button>
          <Button size="sm" variant="outline" className="text-xs gap-1" onClick={() => controlMut.mutate("pause")}><Pause className="h-3 w-3" />Pause</Button>
          <Button size="sm" variant="outline" className="text-xs gap-1" onClick={() => controlMut.mutate("skip")}><SkipForward className="h-3 w-3" />Skip</Button>
          <Button size="sm" variant="outline" className="text-xs gap-1 text-destructive" onClick={() => controlMut.mutate("cancel")}><X className="h-3 w-3" />Cancel</Button>
        </div>
        <Tabs defaultValue="tasks">
          <TabsList className="bg-secondary mb-3"><TabsTrigger value="tasks" className="text-xs">Tasks</TabsTrigger><TabsTrigger value="history" className="text-xs">History</TabsTrigger><TabsTrigger value="context" className="text-xs">Context</TabsTrigger></TabsList>
          <TabsContent value="tasks">
            <div className="space-y-2">
              {tasks.length > 0 ? tasks.map((t: any, i: number) => (
                <div key={i} className={`flex items-center gap-3 p-3 rounded-lg bg-secondary ${t.status === "completed" ? "opacity-60" : ""}`}>
                  <span className="text-xs font-mono text-muted-foreground w-6">#{i + 1}</span>
                  <span className="text-sm flex-1">{t.name}</span>
                  <Badge variant="outline" className={`text-[10px] ${t.status === "completed" ? "text-success" : t.status === "running" ? "text-primary" : ""}`}>{t.status}</Badge>
                </div>
              )) : <p className="text-sm text-muted-foreground text-center py-6">{detail.isLoading ? "Loading..." : "No tasks"}</p>}
            </div>
          </TabsContent>
          <TabsContent value="history">
            <Table><TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">Time</TableHead><TableHead className="text-xs">Task</TableHead><TableHead className="text-xs">Status</TableHead></TableRow></TableHeader>
              <TableBody>{histData.length > 0 ? histData.map((h: any, i: number) => (
                <TableRow key={i} className="border-border/30"><TableCell className="text-xs font-mono">{h.timestamp}</TableCell><TableCell className="text-sm">{h.task}</TableCell><TableCell><Badge variant="outline" className={`text-[10px] ${h.status === "completed" ? "text-success" : "text-destructive"}`}>{h.status}</Badge></TableCell></TableRow>
              )) : <TableRow><TableCell colSpan={3} className="text-center text-sm text-muted-foreground py-6">{history.isLoading ? "Loading..." : "No history"}</TableCell></TableRow>}</TableBody>
            </Table>
          </TabsContent>
          <TabsContent value="context">
            {stack.context ? <pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto">{JSON.stringify(stack.context, null, 2)}</pre> : <p className="text-sm text-muted-foreground">No context data</p>}
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

export default function WorkStacksPage() {
  const [view, setView] = useState<"kanban" | "list">("kanban");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const stacks = useQuery({ queryKey: ["workStacks"], queryFn: getWorkStacks, refetchInterval: 5000 });
  const data = Array.isArray(stacks.data) ? stacks.data : stacks.data?.stacks ?? [];

  const byStatus = (status: string) => data.filter((s: any) => s.status === status);

  return (
    <div className="space-y-6 animate-slide-in">
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div><h1 className="text-2xl font-bold text-foreground">Work Stacks</h1><p className="text-sm text-muted-foreground mt-1">Task stack management & execution</p></div>
        <div className="flex items-center gap-2">
          <div className="flex border border-border rounded-md">
            <Button size="sm" variant={view === "kanban" ? "secondary" : "ghost"} className="h-8 px-2" onClick={() => setView("kanban")}><LayoutGrid className="h-3.5 w-3.5" /></Button>
            <Button size="sm" variant={view === "list" ? "secondary" : "ghost"} className="h-8 px-2" onClick={() => setView("list")}><List className="h-3.5 w-3.5" /></Button>
          </div>
          <Button size="sm" className="gap-1.5 text-xs"><Plus className="h-3.5 w-3.5" />Create Stack</Button>
        </div>
      </div>

      {view === "kanban" ? (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4">
          {statusColumns.map((col) => (
            <div key={col}>
              <div className={`px-3 py-2 rounded-t-lg ${statusColColors[col]}`}>
                <h3 className="text-xs font-semibold uppercase tracking-wider text-foreground/80">{statusLabels[col]}</h3>
                <span className="text-[10px] text-muted-foreground">{byStatus(col).length} items</span>
              </div>
              <div className="space-y-2 pt-2">
                {byStatus(col).map((s: any) => (
                  <Card key={s.id} className="border-border/50 cursor-pointer hover:border-primary/30 transition-colors" onClick={() => setSelectedId(s.id)}>
                    <CardContent className="p-3">
                      <div className="flex items-start justify-between mb-1">
                        <h4 className="text-sm font-medium text-foreground">{s.name}</h4>
                        {s.priority && <Badge variant="outline" className={`text-[10px] ${priorityColors[s.priority] ?? ""}`}>{s.priority}</Badge>}
                      </div>
                      {s.progress != null && <Progress value={s.progress} className="h-1.5 mt-2" />}
                      <div className="flex items-center gap-2 mt-2 text-xs text-muted-foreground">
                        {s.current_task && <span className="truncate">{s.current_task}</span>}
                        {s.agent && <span className="flex items-center gap-1 ml-auto"><Bot className="h-3 w-3" />{s.agent}</span>}
                      </div>
                    </CardContent>
                  </Card>
                ))}
                {byStatus(col).length === 0 && <p className="text-xs text-muted-foreground text-center py-4">Empty</p>}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <Card className="border-border/50"><CardContent className="p-0">
          <Table>
            <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">Name</TableHead><TableHead className="text-xs">Priority</TableHead><TableHead className="text-xs">Status</TableHead><TableHead className="text-xs">Progress</TableHead><TableHead className="text-xs">Agent</TableHead></TableRow></TableHeader>
            <TableBody>
              {data.length > 0 ? data.map((s: any) => (
                <TableRow key={s.id} className="border-border/30 cursor-pointer hover:bg-accent/50" onClick={() => setSelectedId(s.id)}>
                  <TableCell className="text-sm font-medium">{s.name}</TableCell>
                  <TableCell><Badge variant="outline" className={`text-[10px] ${priorityColors[s.priority] ?? ""}`}>{s.priority}</Badge></TableCell>
                  <TableCell><Badge variant="outline" className="text-[10px]">{s.status}</Badge></TableCell>
                  <TableCell>{s.progress != null && <Progress value={s.progress} className="h-1.5 w-20" />}</TableCell>
                  <TableCell className="text-xs text-muted-foreground">{s.agent ?? "—"}</TableCell>
                </TableRow>
              )) : <TableRow><TableCell colSpan={5} className="text-center py-8 text-sm text-muted-foreground">{stacks.isLoading ? "Loading..." : "No stacks"}</TableCell></TableRow>}
            </TableBody>
          </Table>
        </CardContent></Card>
      )}

      <StackDetailDialog stackId={selectedId} open={!!selectedId} onClose={() => setSelectedId(null)} />
    </div>
  );
}
