import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Bug, Play, Pause, SkipForward, StepForward, Square, Eye, Plus, Trash2, CircleDot } from "lucide-react";
import { getDebugExecutions, getDebugTrace, debugControl, getDebugVariables, getDebugBreakpoints, addBreakpoint } from "@/lib/api";

export default function DebuggerPage() {
  const [selectedExec, setSelectedExec] = useState<string | null>(null);
  const [bpDialogOpen, setBpDialogOpen] = useState(false);
  const queryClient = useQueryClient();

  const activeExecs = useQuery({ queryKey: ["debugExecs"], queryFn: getDebugExecutions, refetchInterval: 3000 });
  const trace = useQuery({ queryKey: ["debugTrace", selectedExec], queryFn: () => getDebugTrace(selectedExec!), enabled: !!selectedExec, refetchInterval: 2000 });
  const variables = useQuery({ queryKey: ["debugVars", selectedExec], queryFn: () => getDebugVariables(selectedExec!), enabled: !!selectedExec, refetchInterval: 2000 });
  const breakpoints = useQuery({ queryKey: ["breakpoints"], queryFn: getDebugBreakpoints });

  const controlMut = useMutation({ mutationFn: (action: string) => debugControl(selectedExec!, action) });

  const execList = Array.isArray(activeExecs.data) ? activeExecs.data : activeExecs.data?.executions ?? [];
  const traceData = trace.data ?? {};
  const steps = Array.isArray(traceData.steps) ? traceData.steps : [];
  const vars = variables.data ?? {};
  const bpList = Array.isArray(breakpoints.data) ? breakpoints.data : breakpoints.data?.breakpoints ?? [];

  return (
    <div className="space-y-6 animate-slide-in">
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div><h1 className="text-2xl font-bold text-foreground">Debugger</h1><p className="text-sm text-muted-foreground mt-1">Step-through orchestration debugger</p></div>
        <div className="flex items-center gap-2">
          <Select value={selectedExec ?? ""} onValueChange={setSelectedExec}>
            <SelectTrigger className="h-8 w-64 bg-secondary border-none text-xs"><SelectValue placeholder="Select execution to debug..." /></SelectTrigger>
            <SelectContent>
              {execList.map((e: any) => (
                <SelectItem key={e.id} value={e.id} className="text-xs">{e.name ?? e.id?.slice(0, 12)} — {e.type} ({e.current_step ?? "—"})</SelectItem>
              ))}
              {execList.length === 0 && <SelectItem value="__none" disabled className="text-xs">No active executions</SelectItem>}
            </SelectContent>
          </Select>
        </div>
      </div>

      {selectedExec ? (
        <>
          {/* Debug Controls */}
          <Card className="border-border/50">
            <CardContent className="p-3">
              <div className="flex items-center gap-2 flex-wrap">
                <span className="text-xs text-muted-foreground mr-2">Controls:</span>
                <Button size="sm" variant="outline" className="h-7 text-xs gap-1" onClick={() => controlMut.mutate("pause")}><Pause className="h-3 w-3" />Pause</Button>
                <Button size="sm" variant="outline" className="h-7 text-xs gap-1" onClick={() => controlMut.mutate("resume")}><Play className="h-3 w-3" />Resume</Button>
                <Button size="sm" variant="outline" className="h-7 text-xs gap-1" onClick={() => controlMut.mutate("step_over")}><SkipForward className="h-3 w-3" />Step Over</Button>
                <Button size="sm" variant="outline" className="h-7 text-xs gap-1" onClick={() => controlMut.mutate("step_into")}><StepForward className="h-3 w-3" />Step Into</Button>
                <Button size="sm" variant="outline" className="h-7 text-xs gap-1 text-destructive" onClick={() => controlMut.mutate("stop")}><Square className="h-3 w-3" />Stop</Button>
                {traceData.current_step && (
                  <div className="ml-auto flex items-center gap-2">
                    <Badge variant="outline" className="text-xs text-primary">Current: {traceData.current_step}</Badge>
                    <Badge variant="outline" className="text-xs">{traceData.status ?? "running"}</Badge>
                  </div>
                )}
              </div>
            </CardContent>
          </Card>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* Execution Steps */}
            <Card className="border-border/50">
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium text-muted-foreground">Execution Trace</CardTitle></CardHeader>
              <CardContent className="space-y-1 max-h-96 overflow-auto scrollbar-thin">
                {steps.length > 0 ? steps.map((s: any, i: number) => (
                  <div key={i} className={`flex items-center gap-3 p-2 rounded-lg text-sm ${
                    s.status === "running" ? "bg-primary/10 border border-primary/30" :
                    s.status === "completed" ? "bg-success/5" :
                    s.status === "failed" ? "bg-destructive/5" : "bg-secondary"
                  }`}>
                    <CircleDot className={`h-3 w-3 shrink-0 ${
                      s.status === "running" ? "text-primary animate-pulse" :
                      s.status === "completed" ? "text-success" :
                      s.status === "failed" ? "text-destructive" : "text-muted-foreground"
                    }`} />
                    <span className="flex-1 truncate">{s.name}</span>
                    {s.duration && <span className="text-xs text-muted-foreground">{s.duration}ms</span>}
                    <Badge variant="outline" className="text-[10px]">{s.status}</Badge>
                  </div>
                )) : <p className="text-sm text-muted-foreground text-center py-6">{trace.isLoading ? "Loading..." : "No steps"}</p>}
              </CardContent>
            </Card>

            {/* Inspector */}
            <Card className="border-border/50">
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium text-muted-foreground">Inspector</CardTitle></CardHeader>
              <CardContent>
                <Tabs defaultValue="variables">
                  <TabsList className="bg-secondary mb-3"><TabsTrigger value="variables" className="text-xs">Variables</TabsTrigger><TabsTrigger value="breakpoints" className="text-xs">Breakpoints</TabsTrigger></TabsList>
                  <TabsContent value="variables">
                    {Object.keys(vars).length > 0 ? (
                      <pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto max-h-64">{JSON.stringify(vars, null, 2)}</pre>
                    ) : <p className="text-sm text-muted-foreground text-center py-6">{variables.isLoading ? "Loading..." : "No variables"}</p>}
                  </TabsContent>
                  <TabsContent value="breakpoints">
                    <div className="space-y-2">
                      <Button size="sm" variant="outline" className="text-xs gap-1 mb-2" onClick={() => setBpDialogOpen(true)}><Plus className="h-3 w-3" />Add Breakpoint</Button>
                      {bpList.length > 0 ? bpList.map((bp: any, i: number) => (
                        <div key={i} className="flex items-center gap-2 p-2 rounded-lg bg-secondary text-xs">
                          <CircleDot className="h-3 w-3 text-destructive shrink-0" />
                          <span className="flex-1">{bp.target ?? bp.type}</span>
                          {bp.condition && <span className="text-muted-foreground font-mono">{bp.condition}</span>}
                          <Badge variant="outline" className="text-[10px]">{bp.hits ?? 0} hits</Badge>
                        </div>
                      )) : <p className="text-xs text-muted-foreground text-center py-4">No breakpoints set</p>}
                    </div>
                  </TabsContent>
                </Tabs>
              </CardContent>
            </Card>
          </div>
        </>
      ) : (
        <Card className="border-border/50">
          <CardContent className="flex flex-col items-center justify-center py-24 text-center">
            <div className="h-14 w-14 rounded-2xl bg-primary/10 flex items-center justify-center mb-4"><Bug className="h-7 w-7 text-primary" /></div>
            <h2 className="text-lg font-semibold text-foreground mb-1">Select an Execution</h2>
            <p className="text-sm text-muted-foreground max-w-sm">Choose a running execution from the dropdown above to start debugging.</p>
          </CardContent>
        </Card>
      )}

      {/* Add Breakpoint Dialog */}
      <BreakpointDialog open={bpDialogOpen} onClose={() => setBpDialogOpen(false)} />
    </div>
  );
}

function BreakpointDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [type, setType] = useState("tool");
  const [target, setTarget] = useState("");
  const [condition, setCondition] = useState("");
  const queryClient = useQueryClient();
  const addMut = useMutation({ mutationFn: addBreakpoint, onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["breakpoints"] }); onClose(); } });

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader><DialogTitle>Add Breakpoint</DialogTitle></DialogHeader>
        <div className="space-y-3">
          <div><p className="text-xs text-muted-foreground mb-1">Type</p>
            <Select value={type} onValueChange={setType}><SelectTrigger className="h-8 bg-secondary border-none text-sm"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="tool">Tool</SelectItem><SelectItem value="agent">Agent</SelectItem><SelectItem value="workflow">Workflow Node</SelectItem><SelectItem value="error">Error Condition</SelectItem></SelectContent></Select>
          </div>
          <div><p className="text-xs text-muted-foreground mb-1">Target</p><Input value={target} onChange={(e) => setTarget(e.target.value)} className="h-8 bg-secondary border-none text-sm" placeholder="Tool or node name..." /></div>
          <div><p className="text-xs text-muted-foreground mb-1">Condition (optional)</p><Input value={condition} onChange={(e) => setCondition(e.target.value)} className="h-8 bg-secondary border-none text-sm font-mono" placeholder='e.g. input.user_id == "123"' /></div>
          <Button size="sm" onClick={() => addMut.mutate({ type, target, condition: condition || undefined })} disabled={!target || addMut.isPending}>
            {addMut.isPending ? "Adding..." : "Add Breakpoint"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
