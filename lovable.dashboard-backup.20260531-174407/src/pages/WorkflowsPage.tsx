import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Search, Play, Copy, Trash2, Clock, GitBranch, Pause, Plus } from "lucide-react";
import { getWorkflows, runWorkflow, deleteWorkflow } from "@/lib/api";

const statusColors: Record<string, string> = {
  active: "text-success border-success/20 bg-success/10",
  draft: "text-muted-foreground",
  paused: "text-warning border-warning/20 bg-warning/10",
};
const triggerColors: Record<string, string> = {
  manual: "bg-info/10 text-info border-info/20",
  schedule: "bg-warning/10 text-warning border-warning/20",
  webhook: "bg-primary/10 text-primary border-primary/20",
};

export default function WorkflowsPage() {
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState("all");
  const queryClient = useQueryClient();
  const workflows = useQuery({ queryKey: ["workflows"], queryFn: getWorkflows, refetchInterval: 10000 });
  const runMut = useMutation({ mutationFn: runWorkflow, onSuccess: () => queryClient.invalidateQueries({ queryKey: ["workflows"] }) });
  const deleteMut = useMutation({ mutationFn: deleteWorkflow, onSuccess: () => queryClient.invalidateQueries({ queryKey: ["workflows"] }) });

  const data = Array.isArray(workflows.data) ? workflows.data : workflows.data?.workflows ?? [];
  const filtered = data.filter((w: any) => {
    if (statusFilter !== "all" && w.status !== statusFilter) return false;
    if (search && !w.name?.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  return (
    <div className="space-y-6 animate-slide-in">
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div><h1 className="text-2xl font-bold text-foreground">Workflows</h1><p className="text-sm text-muted-foreground mt-1">Visual workflow builder & execution</p></div>
        <div className="flex items-center gap-2">
          <div className="relative">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
            <Input placeholder="Search..." value={search} onChange={(e) => setSearch(e.target.value)} className="h-8 w-48 pl-8 bg-secondary border-none text-sm" />
          </div>
          <Select value={statusFilter} onValueChange={setStatusFilter}><SelectTrigger className="h-8 w-28 bg-secondary border-none text-xs"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="all">All</SelectItem><SelectItem value="active">Active</SelectItem><SelectItem value="draft">Draft</SelectItem><SelectItem value="paused">Paused</SelectItem></SelectContent></Select>
          <Button size="sm" className="gap-1.5 text-xs"><Plus className="h-3.5 w-3.5" />Create</Button>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {filtered.length > 0 ? filtered.map((w: any) => (
          <Card key={w.id ?? w.name} className="border-border/50 card-glow">
            <CardContent className="p-4">
              <div className="flex items-start justify-between mb-2">
                <div className="flex items-center gap-2">
                  <div className="p-2 rounded-lg bg-primary/10 text-primary"><GitBranch className="h-4 w-4" /></div>
                  <div><h3 className="text-sm font-semibold text-foreground">{w.name}</h3></div>
                </div>
                <Badge variant="outline" className={`text-[10px] ${statusColors[w.status] ?? ""}`}>{w.status}</Badge>
              </div>
              <div className="flex items-center gap-2 mt-2">
                {w.trigger && <Badge variant="outline" className={`text-[10px] ${triggerColors[w.trigger] ?? ""}`}>{w.trigger}</Badge>}
              </div>
              <div className="flex items-center gap-4 mt-3 text-xs text-muted-foreground">
                {w.last_run && <span className="flex items-center gap-1"><Clock className="h-3 w-3" />{w.last_run}</span>}
                {w.success_rate != null && <span>{w.success_rate}% success</span>}
                {w.avg_duration && <span>{w.avg_duration}s avg</span>}
              </div>
              <div className="flex items-center gap-1 mt-3 pt-3 border-t border-border/30">
                <Button size="sm" variant="ghost" className="h-7 text-xs gap-1 text-muted-foreground hover:text-success" onClick={() => runMut.mutate(w.id)} disabled={runMut.isPending}>
                  <Play className="h-3 w-3" />Run
                </Button>
                <Button size="sm" variant="ghost" className="h-7 text-xs gap-1 text-muted-foreground"><Copy className="h-3 w-3" />Clone</Button>
                <Button size="sm" variant="ghost" className="h-7 text-xs gap-1 text-muted-foreground hover:text-destructive ml-auto" onClick={() => deleteMut.mutate(w.id)}>
                  <Trash2 className="h-3 w-3" />
                </Button>
              </div>
            </CardContent>
          </Card>
        )) : (
          <div className="col-span-full text-center py-12 text-sm text-muted-foreground">
            {workflows.isLoading ? "Loading workflows..." : "No workflows found"}
          </div>
        )}
      </div>
    </div>
  );
}
