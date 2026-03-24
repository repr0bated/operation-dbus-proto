import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Copy, Shield } from "lucide-react";
import { getVpnConnections, getVpnConfig } from "@/lib/api";
import { useToast } from "@/hooks/use-toast";

export default function VpnPage() {
  const { toast } = useToast();
  const connections = useQuery({ queryKey: ["vpnConnections"], queryFn: getVpnConnections, refetchInterval: 5000 });
  const config = useQuery({ queryKey: ["vpnConfig"], queryFn: getVpnConfig });

  const connData = Array.isArray(connections.data) ? connections.data : connections.data?.connections ?? [];

  const copyText = (text: string) => {
    navigator.clipboard.writeText(text);
    toast({ title: "Copied to clipboard" });
  };

  return (
    <div className="space-y-6 animate-slide-in">
      <div>
        <h1 className="text-2xl font-bold text-foreground">VPN Status</h1>
        <p className="text-sm text-muted-foreground mt-1">WireGuard connections & configuration</p>
      </div>

      {/* Active Connections */}
      <Card className="border-border/50">
        <CardHeader className="pb-2">
          <CardTitle className="text-sm font-medium text-muted-foreground">Active Connections</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow className="border-border/50 hover:bg-transparent">
                <TableHead className="text-xs">User</TableHead>
                <TableHead className="text-xs">IP</TableHead>
                <TableHead className="text-xs">Connected</TableHead>
                <TableHead className="text-xs">Upload</TableHead>
                <TableHead className="text-xs">Download</TableHead>
                <TableHead className="text-xs">Latency</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {connData.length > 0 ? connData.map((c: any, i: number) => (
                <TableRow key={i} className="border-border/30">
                  <TableCell className="text-sm">{c.user ?? c.email}</TableCell>
                  <TableCell className="text-sm font-mono text-muted-foreground">{c.ip}</TableCell>
                  <TableCell className="text-sm text-muted-foreground">{c.connected_since ?? c.duration}</TableCell>
                  <TableCell className="text-sm text-muted-foreground">{c.upload}</TableCell>
                  <TableCell className="text-sm text-muted-foreground">{c.download}</TableCell>
                  <TableCell className="text-sm font-mono text-success">{c.latency}</TableCell>
                </TableRow>
              )) : (
                <TableRow>
                  <TableCell colSpan={6} className="text-center py-8 text-sm text-muted-foreground">
                    {connections.isLoading ? "Loading..." : "No active connections"}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      {/* Server Config */}
      <Card className="border-border/50">
        <CardHeader className="pb-2">
          <div className="flex items-center gap-2">
            <Shield className="h-4 w-4 text-primary" />
            <CardTitle className="text-sm font-medium text-muted-foreground">Server Configuration</CardTitle>
          </div>
        </CardHeader>
        <CardContent>
          {config.data ? (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {Object.entries(config.data).map(([key, value]) => (
                <div key={key} className="flex items-center justify-between p-3 rounded-lg bg-secondary">
                  <div>
                    <p className="text-xs text-muted-foreground capitalize">{key.replace(/_/g, " ")}</p>
                    <p className="text-sm font-mono text-foreground mt-0.5">{String(value)}</p>
                  </div>
                  <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-muted-foreground" onClick={() => copyText(String(value))}>
                    <Copy className="h-3.5 w-3.5" />
                  </Button>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">{config.isLoading ? "Loading..." : "Config unavailable"}</p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
