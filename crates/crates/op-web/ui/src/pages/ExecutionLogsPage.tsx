import { useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Search, Download, RefreshCw, ScrollText, Eye } from "lucide-react";
import { getExecutionLogs, replayExecution } from "@/lib/api";

export default function ExecutionLogsPage() {
  const [search, setSearch] = useState("");
  const [typeFilter, setTypeFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [selectedLog, setSelectedLog] = useState<any>(null);

  const logs = useQuery({
    queryKey: ["executionLogs", typeFilter, statusFilter],
    queryFn: () => {
      const filters: Record<string, string> = {};
      if (typeFilter !== "all") filters.type = typeFilter;
      if (statusFilter !== "all") filters.status = statusFilter;
      return getExecutionLogs(Object.keys(filters).length ? filters : undefined);
    },
    refetchInterval: 5000,
  });

  const replayMut = useMutation({ mutationFn: replayExecution });

  const data = Array.isArray(logs.data) ? logs.data : logs.data?.logs ?? [];
  const filtered = data.filter((l: any) => !search || l.name?.toLowerCase().includes(search.toLowerCase()) || l.input?.toLowerCase().includes(search.toLowerCase()));

  const exportLogs = (format: "csv" | "json") => {
    const content = format === "json" ? JSON.stringify(filtered, null, 2) : [
      "timestamp,type,name,status,duration",
      ...filtered.map((l: any) => `${l.timestamp},${l.type},${l.name},${l.status},${l.duration}`)
    ].join("\n");
    const blob = new Blob([content], { type: format === "json" ? "application/json" : "text/csv" });
    const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = `logs.${format}`; a.click();
  };

  return (
    <div className="space-y-6 animate-slide-in">
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div><h1 className="text-2xl font-bold text-foreground">Execution Logs</h1><p className="text-sm text-muted-foreground mt-1">Unified log viewer for tools, agents & workflows</p></div>
        <div className="flex items-center gap-2">
          <div className="relative">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
            <Input placeholder="Search..." value={search} onChange={(e) => setSearch(e.target.value)} className="h-8 w-48 pl-8 bg-secondary border-none text-sm" />
          </div>
          <Select value={typeFilter} onValueChange={setTypeFilter}><SelectTrigger className="h-8 w-24 bg-secondary border-none text-xs"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="all">All Types</SelectItem><SelectItem value="tool">Tool</SelectItem><SelectItem value="agent">Agent</SelectItem><SelectItem value="workflow">Workflow</SelectItem></SelectContent></Select>
          <Select value={statusFilter} onValueChange={setStatusFilter}><SelectTrigger className="h-8 w-24 bg-secondary border-none text-xs"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="all">All Status</SelectItem><SelectItem value="success">Success</SelectItem><SelectItem value="failed">Failed</SelectItem><SelectItem value="warning">Warning</SelectItem></SelectContent></Select>
          <Button size="sm" variant="outline" className="h-8 text-xs gap-1" onClick={() => exportLogs("json")}><Download className="h-3 w-3" />Export</Button>
        </div>
      </div>

      <Card className="border-border/50">
        <CardContent className="p-0">
          <Table>
            <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">Time</TableHead><TableHead className="text-xs">Type</TableHead><TableHead className="text-xs">Name</TableHead><TableHead className="text-xs">Input</TableHead><TableHead className="text-xs">Duration</TableHead><TableHead className="text-xs">Status</TableHead><TableHead className="text-xs text-right">Actions</TableHead></TableRow></TableHeader>
            <TableBody>
              {filtered.length > 0 ? filtered.slice(0, 50).map((l: any, i: number) => (
                <TableRow key={l.id ?? i} className="border-border/30">
                  <TableCell className="text-xs font-mono">{l.timestamp}</TableCell>
                  <TableCell><Badge variant="outline" className="text-[10px]">{l.type}</Badge></TableCell>
                  <TableCell className="text-sm">{l.name}</TableCell>
                  <TableCell className="text-xs text-muted-foreground max-w-[200px] truncate">{typeof l.input === "string" ? l.input : JSON.stringify(l.input)?.slice(0, 60)}</TableCell>
                  <TableCell className="text-xs text-muted-foreground">{l.duration}ms</TableCell>
                  <TableCell><Badge variant="outline" className={`text-[10px] ${l.status === "success" ? "text-success" : l.status === "failed" ? "text-destructive" : "text-warning"}`}>{l.status}</Badge></TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1">
                      <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={() => setSelectedLog(l)}><Eye className="h-3 w-3" /></Button>
                      <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={() => replayMut.mutate(l.id)} disabled={replayMut.isPending}><RefreshCw className="h-3 w-3" /></Button>
                    </div>
                  </TableCell>
                </TableRow>
              )) : <TableRow><TableCell colSpan={7} className="text-center py-8 text-sm text-muted-foreground">{logs.isLoading ? "Loading..." : "No logs found"}</TableCell></TableRow>}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Dialog open={!!selectedLog} onOpenChange={(v) => !v && setSelectedLog(null)}>
        <DialogContent className="max-w-2xl max-h-[80vh] overflow-auto">
          <DialogHeader><DialogTitle className="flex items-center gap-2"><ScrollText className="h-4 w-4" />Execution Detail</DialogTitle></DialogHeader>
          {selectedLog && (
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">ID</p><p className="text-sm font-mono">{selectedLog.id ?? "—"}</p></div>
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Type</p><p className="text-sm">{selectedLog.type}</p></div>
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Duration</p><p className="text-sm">{selectedLog.duration}ms</p></div>
                <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Status</p><Badge variant="outline" className={`text-xs ${selectedLog.status === "success" ? "text-success" : "text-destructive"}`}>{selectedLog.status}</Badge></div>
              </div>
              <div><p className="text-xs font-medium text-muted-foreground mb-1">Input</p><pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto max-h-40">{typeof selectedLog.input === "string" ? selectedLog.input : JSON.stringify(selectedLog.input, null, 2)}</pre></div>
              <div><p className="text-xs font-medium text-muted-foreground mb-1">Output</p><pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto max-h-40">{typeof selectedLog.output === "string" ? selectedLog.output : JSON.stringify(selectedLog.output, null, 2)}</pre></div>
              {selectedLog.error && <div><p className="text-xs font-medium text-destructive mb-1">Error</p><pre className="text-xs font-mono bg-destructive/5 p-3 rounded-lg overflow-auto">{selectedLog.error}</pre></div>}
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
