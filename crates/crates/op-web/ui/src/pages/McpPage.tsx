import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Zap, Clock, Wrench, Brain, Database, Trash2, RefreshCw } from "lucide-react";
import {
  getMcpServers,
  getMcpAgents,
  setMcpAgents,
  getMcpMemoryStats,
  queryMcpMemory,
  deleteMcpMemory
} from "@/lib/api";

export default function McpPage() {
  const queryClient = useQueryClient();
  const servers = useQuery({ queryKey: ["mcpServers"], queryFn: getMcpServers, refetchInterval: 10000 });
  const agents = useQuery({ queryKey: ["mcpAgents"], queryFn: getMcpAgents });
  const memoryStats = useQuery({ queryKey: ["mcpMemoryStats"], queryFn: getMcpMemoryStats, refetchInterval: 5000 });

  const data = Array.isArray(servers.data) ? servers.data : servers.data?.servers ?? [];
  const agentList = Array.isArray(agents.data) ? agents.data : [];
  const stats = memoryStats.data ?? {};

  const toggleAgent = useMutation({
    mutationFn: (agentId: string) => {
      const agent = agentList.find((a: any) => a.id === agentId);
      const currentEnabled = agentList.filter((a: any) => a.enabled).map((a: any) => a.id);
      const newEnabled = agent?.enabled
        ? currentEnabled.filter((id: string) => id !== agentId)
        : [...currentEnabled, agentId];
      return setMcpAgents(newEnabled);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["mcpAgents"] });
      queryClient.invalidateQueries({ queryKey: ["mcpServers"] });
    },
  });

  return (
    <div className="space-y-6 animate-slide-in">
      <div>
        <h1 className="text-2xl font-bold text-foreground">MCP Services</h1>
        <p className="text-sm text-muted-foreground mt-1">Model Context Protocol server and agent management</p>
      </div>

      <Tabs defaultValue="servers" className="space-y-4">
        <TabsList className="bg-secondary">
          <TabsTrigger value="servers" className="text-sm">Servers</TabsTrigger>
          <TabsTrigger value="agents" className="text-sm">Cognitive Agents</TabsTrigger>
          <TabsTrigger value="memory" className="text-sm">Memory Store</TabsTrigger>
        </TabsList>

        <TabsContent value="servers" className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {data.length > 0 ? data.map((s: any) => (
              <Card key={s.id} className={`border-border/50 card-glow ${s.status === "running" ? "" : "opacity-60"}`}>
                <CardContent className="p-5">
                  <div className="flex items-start justify-between mb-3">
                    <div className="flex items-center gap-2">
                      <div className={`p-2 rounded-lg ${s.status === "running" ? "bg-success/10 text-success" : "bg-muted text-muted-foreground"}`}>
                        <Zap className="h-4 w-4" />
                      </div>
                      <div>
                        <h3 className="text-sm font-semibold text-foreground">{s.name}</h3>
                        <p className="text-xs text-muted-foreground">{s.server_type}</p>
                      </div>
                    </div>
                    <Badge variant="outline" className={`text-xs ${s.status === "running" ? "text-success border-success/30" : "text-destructive border-destructive/30"}`}>
                      {s.status}
                    </Badge>
                  </div>
                  <div className="space-y-2">
                    <div className="flex items-center gap-4 text-xs text-muted-foreground">
                      <span className="flex items-center gap-1"><Wrench className="h-3 w-3" /> {s.tools_count} tools</span>
                      <span className="text-xs font-mono">{s.url}</span>
                    </div>
                    {s.agents && (
                      <div className="pt-2 border-t border-border/30">
                        <p className="text-xs text-muted-foreground mb-1">Active Agents:</p>
                        <div className="flex flex-wrap gap-1">
                          {s.agents.map((agent: string) => (
                            <Badge key={agent} variant="secondary" className="text-xs">{agent}</Badge>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                </CardContent>
              </Card>
            )) : (
              <div className="col-span-full text-center py-12 text-sm text-muted-foreground">
                {servers.isLoading ? "Loading MCP servers..." : "No MCP servers found"}
              </div>
            )}
          </div>
        </TabsContent>

        <TabsContent value="agents" className="space-y-4">
          <Card className="border-border/50">
            <CardHeader>
              <CardTitle className="text-lg flex items-center gap-2">
                <Brain className="h-5 w-5" />
                Cognitive Agents
              </CardTitle>
              <p className="text-sm text-muted-foreground">Enable or disable agents for the cognitive MCP server</p>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                {agentList.length > 0 ? agentList.map((agent: any) => (
                  <div key={agent.id} className="flex items-start justify-between p-3 rounded-lg border border-border/50 hover:border-primary/30 transition-colors">
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-1">
                        <h4 className="text-sm font-medium">{agent.name}</h4>
                        <Badge variant={agent.enabled ? "default" : "secondary"} className="text-xs">
                          {agent.enabled ? "Enabled" : "Disabled"}
                        </Badge>
                      </div>
                      <p className="text-xs text-muted-foreground mb-2">{agent.description}</p>
                      <div className="flex flex-wrap gap-1">
                        {agent.capabilities.map((cap: string) => (
                          <Badge key={cap} variant="outline" className="text-xs">{cap}</Badge>
                        ))}
                      </div>
                    </div>
                    <Switch
                      checked={agent.enabled}
                      onCheckedChange={() => toggleAgent.mutate(agent.id)}
                      disabled={toggleAgent.isPending}
                    />
                  </div>
                )) : (
                  <div className="text-center py-8 text-sm text-muted-foreground">
                    {agents.isLoading ? "Loading agents..." : "No agents available"}
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="memory" className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
            <Card className="border-border/50">
              <CardContent className="p-4">
                <div className="flex items-center gap-2 mb-1">
                  <Database className="h-4 w-4 text-primary" />
                  <p className="text-xs text-muted-foreground">Total Entries</p>
                </div>
                <p className="text-2xl font-bold">{stats.total_entries ?? 0}</p>
              </CardContent>
            </Card>
            <Card className="border-border/50">
              <CardContent className="p-4">
                <p className="text-xs text-muted-foreground mb-1">Ephemeral</p>
                <p className="text-2xl font-bold text-yellow-500">{stats.ephemeral ?? 0}</p>
              </CardContent>
            </Card>
            <Card className="border-border/50">
              <CardContent className="p-4">
                <p className="text-xs text-muted-foreground mb-1">Persistent</p>
                <p className="text-2xl font-bold text-green-500">{stats.persistent ?? 0}</p>
              </CardContent>
            </Card>
            <Card className="border-border/50">
              <CardContent className="p-4">
                <p className="text-xs text-muted-foreground mb-1">Shared</p>
                <p className="text-2xl font-bold text-blue-500">{stats.shared ?? 0}</p>
              </CardContent>
            </Card>
          </div>

          <Card className="border-border/50">
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle className="text-lg">Memory Statistics</CardTitle>
                <Button size="sm" variant="outline" onClick={() => queryClient.invalidateQueries({ queryKey: ["mcpMemoryStats"] })}>
                  <RefreshCw className="h-3 w-3 mr-1" />
                  Refresh
                </Button>
              </div>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 gap-4">
                <div className="p-3 rounded-lg bg-secondary">
                  <p className="text-xs text-muted-foreground mb-1">Total Memory</p>
                  <p className="text-sm font-medium">{stats.total_memory_bytes ? (stats.total_memory_bytes / 1024).toFixed(2) + ' KB' : '—'}</p>
                </div>
                <div className="p-3 rounded-lg bg-secondary">
                  <p className="text-xs text-muted-foreground mb-1">Max Entries</p>
                  <p className="text-sm font-medium">{stats.max_entries ?? '—'}</p>
                </div>
                <div className="p-3 rounded-lg bg-secondary">
                  <p className="text-xs text-muted-foreground mb-1">Oldest Entry</p>
                  <p className="text-sm font-medium">{stats.oldest_entry ? new Date(stats.oldest_entry).toLocaleDateString() : '—'}</p>
                </div>
                <div className="p-3 rounded-lg bg-secondary">
                  <p className="text-xs text-muted-foreground mb-1">Most Accessed</p>
                  <p className="text-sm font-medium font-mono">{stats.most_accessed_key ?? '—'}</p>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
