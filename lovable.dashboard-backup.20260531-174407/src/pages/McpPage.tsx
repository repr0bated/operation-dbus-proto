import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { Zap, Clock, Wrench, Settings, RefreshCw, Power } from "lucide-react";
import { getMcpServers, getMcpServerDetail, getMcpServerTools, getMcpServerConfig, getMcpServerLogs, restartMcpServer } from "@/lib/api";
import { useMutation, useQueryClient } from "@tanstack/react-query";

function McpServerDetailDialog({ server, open, onClose }: { server: any; open: boolean; onClose: () => void }) {
  const queryClient = useQueryClient();
  const serverId = server?.id ?? server?.name;
  const detail = useQuery({ queryKey: ["mcpDetail", serverId], queryFn: () => getMcpServerDetail(serverId), enabled: !!serverId });
  const tools = useQuery({ queryKey: ["mcpTools", serverId], queryFn: () => getMcpServerTools(serverId), enabled: !!serverId });
  const config = useQuery({ queryKey: ["mcpConfig", serverId], queryFn: () => getMcpServerConfig(serverId), enabled: !!serverId });
  const logs = useQuery({ queryKey: ["mcpLogs", serverId], queryFn: () => getMcpServerLogs(serverId), enabled: !!serverId });
  const restartMut = useMutation({ mutationFn: () => restartMcpServer(serverId), onSuccess: () => queryClient.invalidateQueries({ queryKey: ["mcpServers"] }) });

  const info = detail.data ?? server ?? {};
  const toolList = Array.isArray(tools.data) ? tools.data : tools.data?.tools ?? [];
  const logList = Array.isArray(logs.data) ? logs.data : logs.data?.logs ?? [];

  if (!server) return null;

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {info.name}
            <Badge variant="outline" className={`text-xs ${info.status === "online" ? "text-success border-success/30" : "text-destructive border-destructive/30"}`}>{info.status}</Badge>
          </DialogTitle>
        </DialogHeader>
        <div className="flex gap-2 mb-3">
          <Button size="sm" variant="outline" className="text-xs gap-1" onClick={() => restartMut.mutate()} disabled={restartMut.isPending}>
            <RefreshCw className="h-3 w-3" />{restartMut.isPending ? "Restarting..." : "Restart"}
          </Button>
          <Button size="sm" variant="outline" className="text-xs gap-1 text-destructive"><Power className="h-3 w-3" />Disable</Button>
        </div>
        <Tabs defaultValue="overview">
          <TabsList className="bg-secondary mb-3">
            <TabsTrigger value="overview" className="text-xs">Overview</TabsTrigger>
            <TabsTrigger value="tools" className="text-xs">Tools</TabsTrigger>
            <TabsTrigger value="config" className="text-xs">Configuration</TabsTrigger>
            <TabsTrigger value="logs" className="text-xs">Logs</TabsTrigger>
          </TabsList>
          <TabsContent value="overview" className="space-y-3">
            <p className="text-sm text-muted-foreground">{info.description ?? "No description available"}</p>
            <div className="grid grid-cols-2 gap-3">
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Endpoint</p><p className="text-sm font-mono">{info.endpoint ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Tools</p><p className="text-sm font-medium">{info.tools ?? toolList.length}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Response Time</p><p className="text-sm">{info.response_time ?? "—"}ms</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Last Request</p><p className="text-sm">{info.last_request ?? "—"}</p></div>
            </div>
          </TabsContent>
          <TabsContent value="tools">
            <Table>
              <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">Name</TableHead><TableHead className="text-xs">Description</TableHead><TableHead className="text-xs">Last Used</TableHead></TableRow></TableHeader>
              <TableBody>
                {toolList.length > 0 ? toolList.map((t: any, i: number) => (
                  <TableRow key={i} className="border-border/30"><TableCell className="text-sm font-medium">{t.name}</TableCell><TableCell className="text-sm text-muted-foreground">{t.description}</TableCell><TableCell className="text-xs text-muted-foreground">{t.last_used ?? "—"}</TableCell></TableRow>
                )) : <TableRow><TableCell colSpan={3} className="text-center py-6 text-sm text-muted-foreground">{tools.isLoading ? "Loading..." : "No tools"}</TableCell></TableRow>}
              </TableBody>
            </Table>
          </TabsContent>
          <TabsContent value="config">
            <pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto max-h-64">
              {config.data ? JSON.stringify(config.data, null, 2) : (config.isLoading ? "Loading..." : "No configuration")}
            </pre>
          </TabsContent>
          <TabsContent value="logs">
            <div className="space-y-1 max-h-64 overflow-auto scrollbar-thin">
              {logList.length > 0 ? logList.map((l: any, i: number) => (
                <div key={i} className="text-xs font-mono py-0.5 flex gap-2">
                  <span className="text-muted-foreground/60">{l.timestamp?.slice(11, 19) ?? ""}</span>
                  <span className={l.level === "ERROR" ? "text-destructive" : "text-foreground/80"}>{l.message}</span>
                </div>
              )) : <p className="text-sm text-muted-foreground text-center py-6">{logs.isLoading ? "Loading..." : "No logs"}</p>}
            </div>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

export default function McpPage() {
  const [selectedServer, setSelectedServer] = useState<any>(null);
  const servers = useQuery({ queryKey: ["mcpServers"], queryFn: getMcpServers, refetchInterval: 10000 });
  const data = Array.isArray(servers.data) ? servers.data : servers.data?.servers ?? [];

  return (
    <div className="space-y-6 animate-slide-in">
      <div>
        <h1 className="text-2xl font-bold text-foreground">MCP Services</h1>
        <p className="text-sm text-muted-foreground mt-1">Model Context Protocol server management</p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {data.length > 0 ? data.map((s: any) => (
          <Card key={s.id ?? s.name} className={`border-border/50 card-glow cursor-pointer hover:border-primary/30 transition-colors ${s.status === "online" ? "" : "opacity-60"}`} onClick={() => setSelectedServer(s)}>
            <CardContent className="p-5">
              <div className="flex items-start justify-between mb-3">
                <div className="flex items-center gap-2">
                  <div className={`p-2 rounded-lg ${s.status === "online" ? "bg-success/10 text-success" : s.status === "degraded" ? "bg-warning/10 text-warning" : "bg-muted text-muted-foreground"}`}>
                    <Zap className="h-4 w-4" />
                  </div>
                  <div>
                    <h3 className="text-sm font-semibold text-foreground">{s.name}</h3>
                    <p className="text-xs text-muted-foreground">{s.description ?? ""}</p>
                  </div>
                </div>
                <Badge variant="outline" className={`text-xs ${s.status === "online" ? "text-success border-success/30" : s.status === "degraded" ? "text-warning border-warning/30" : "text-destructive border-destructive/30"}`}>
                  {s.status}
                </Badge>
              </div>
              <div className="flex items-center gap-4 text-xs text-muted-foreground">
                <span className="flex items-center gap-1"><Wrench className="h-3 w-3" /> {s.tools ?? 0} tools</span>
                {s.response_time != null && <span className="flex items-center gap-1"><Clock className="h-3 w-3" /> {s.response_time}ms</span>}
                {s.last_request && <span className="flex items-center gap-1"><Clock className="h-3 w-3" /> {s.last_request}</span>}
              </div>
            </CardContent>
          </Card>
        )) : (
          <div className="col-span-full text-center py-12 text-sm text-muted-foreground">
            {servers.isLoading ? "Loading MCP servers..." : "No MCP servers found"}
          </div>
        )}
      </div>

      <McpServerDetailDialog server={selectedServer} open={!!selectedServer} onClose={() => setSelectedServer(null)} />
    </div>
  );
}
