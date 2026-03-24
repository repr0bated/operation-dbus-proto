import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { Shield, Activity, Mail, Zap, ArrowUpRight, ArrowDownRight, Users, Globe } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  LineChart, Line, AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer,
} from "recharts";
import { getPrivacyStatus, getHealth, getMailStats, getMcpStatus, getConnectionHistory, getHealthMetrics, getRecentActivity } from "@/lib/api";

function MetricCard({ title, value, subtitle, icon: Icon, trend, color, onClick }: {
  title: string; value: string | number; subtitle?: string; icon: any; trend?: "up" | "down"; color: string; onClick?: () => void;
}) {
  return (
    <Card className={`card-glow border-border/50 ${onClick ? "cursor-pointer hover:border-primary/30 transition-colors" : ""}`} onClick={onClick}>
      <CardContent className="p-5">
        <div className="flex items-start justify-between">
          <div>
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{title}</p>
            <p className="text-3xl font-bold mt-1 text-foreground">{value}</p>
            {subtitle && (
              <div className="flex items-center gap-1 mt-1">
                {trend === "up" && <ArrowUpRight className="h-3 w-3 text-success" />}
                {trend === "down" && <ArrowDownRight className="h-3 w-3 text-destructive" />}
                <span className="text-xs text-muted-foreground">{subtitle}</span>
              </div>
            )}
          </div>
          <div className={`p-2.5 rounded-lg ${color}`}>
            <Icon className="h-5 w-5" />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

const activityIcons: Record<string, any> = { user: Users, vpn: Shield, mail: Mail, mcp: Zap, agent: Activity, workflow: Activity };
const activityColors: Record<string, string> = {
  user: "text-info", vpn: "text-success", mail: "text-warning", mcp: "text-primary", agent: "text-primary", workflow: "text-info",
};

export default function Dashboard() {
  const navigate = useNavigate();
  const privacy = useQuery({ queryKey: ["privacy"], queryFn: getPrivacyStatus, refetchInterval: 10000 });
  const health = useQuery({ queryKey: ["health"], queryFn: getHealth, refetchInterval: 10000 });
  const mail = useQuery({ queryKey: ["mailStats"], queryFn: getMailStats, refetchInterval: 10000 });
  const mcp = useQuery({ queryKey: ["mcpStatus"], queryFn: getMcpStatus, refetchInterval: 10000 });
  const connections = useQuery({ queryKey: ["connections"], queryFn: getConnectionHistory });
  const metrics = useQuery({ queryKey: ["healthMetrics"], queryFn: getHealthMetrics });
  const activity = useQuery({ queryKey: ["activity"], queryFn: getRecentActivity, refetchInterval: 5000 });

  const healthStatus = health.data?.status ?? (health.data?.cpu_usage != null ? (health.data.cpu_usage > 90 ? "degraded" : "healthy") : undefined);
  const healthBadgeColor = healthStatus === "healthy" ? "bg-success/10 text-success border-success/20" : healthStatus === "degraded" ? "bg-warning/10 text-warning border-warning/20" : "bg-destructive/10 text-destructive border-destructive/20";

  return (
    <div className="space-y-6 animate-slide-in">
      <div>
        <h1 className="text-2xl font-bold text-foreground">Dashboard</h1>
        <p className="text-sm text-muted-foreground mt-1">Operation-DBUS system overview</p>
      </div>

      {/* Metric Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <MetricCard
          title="Active VPN Users"
          value={privacy.data?.registered_users ?? "—"}
          subtitle={privacy.data?.available ? "+12% from yesterday" : "Server offline"}
          icon={Shield}
          trend="up"
          color="bg-primary/10 text-primary"
          onClick={() => navigate("/vpn")}
        />
        <div>
          <Card className="card-glow border-border/50">
            <CardContent className="p-5">
              <div className="flex items-start justify-between">
                <div>
                  <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">System Health</p>
                  {healthStatus && <Badge variant="outline" className={`text-xs mt-1 ${healthBadgeColor}`}>{healthStatus}</Badge>}
                  <div className="flex gap-3 mt-2 text-xs text-muted-foreground">
                    {health.data?.cpu_usage != null && <span>CPU: {health.data.cpu_usage}%</span>}
                    {health.data?.memory_usage != null && <span>Mem: {health.data.memory_usage}%</span>}
                    {health.data?.disk_usage != null && <span>Disk: {health.data.disk_usage}%</span>}
                  </div>
                  {health.data?.uptime && <p className="text-xs text-muted-foreground mt-1">Uptime: {health.data.uptime}</p>}
                </div>
                <div className="p-2.5 rounded-lg bg-success/10 text-success"><Activity className="h-5 w-5" /></div>
              </div>
            </CardContent>
          </Card>
        </div>
        <MetricCard
          title="Mail Server"
          value={mail.data?.sent_today ?? "—"}
          subtitle={mail.data ? `${mail.data.received_today ?? 0} received • ${mail.data.queue_count ?? 0} queued` : undefined}
          icon={Mail}
          trend="up"
          color="bg-warning/10 text-warning"
          onClick={() => navigate("/mail")}
        />
        <MetricCard
          title="MCP Services"
          value={mcp.data ? `${mcp.data.active_servers ?? mcp.data.active ?? "—"}/${mcp.data.total_servers ?? mcp.data.total ?? "—"}` : "—"}
          subtitle={mcp.data?.avg_response_time ? `${mcp.data.avg_response_time}ms avg` : undefined}
          icon={Zap}
          color="bg-info/10 text-info"
          onClick={() => navigate("/mcp")}
        />
      </div>

      {/* Charts */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card className="border-border/50">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">VPN Connections (24h)</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-64">
              {connections.data ? (
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={Array.isArray(connections.data) ? connections.data : []}>
                    <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                    <XAxis dataKey="hour" tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }} />
                    <YAxis tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }} />
                    <Tooltip contentStyle={{ background: "hsl(var(--card))", border: "1px solid hsl(var(--border))", borderRadius: "8px", fontSize: 12 }} />
                    <Line type="monotone" dataKey="connections" stroke="hsl(var(--primary))" strokeWidth={2} dot={false} />
                  </LineChart>
                </ResponsiveContainer>
              ) : (
                <div className="h-full flex items-center justify-center text-muted-foreground text-sm">
                  {connections.isLoading ? "Loading..." : "No data available"}
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        <Card className="border-border/50">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">System Resources (1h)</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-64">
              {metrics.data ? (
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={Array.isArray(metrics.data) ? metrics.data : []}>
                    <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                    <XAxis dataKey="hour" tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }} />
                    <YAxis tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }} domain={[0, 100]} />
                    <Tooltip contentStyle={{ background: "hsl(var(--card))", border: "1px solid hsl(var(--border))", borderRadius: "8px", fontSize: 12 }} />
                    <Area type="monotone" dataKey="cpu" stackId="1" stroke="hsl(var(--primary))" fill="hsl(var(--primary) / 0.2)" name="CPU" />
                    <Area type="monotone" dataKey="memory" stackId="1" stroke="hsl(var(--success))" fill="hsl(var(--success) / 0.2)" name="Memory" />
                    <Area type="monotone" dataKey="network" stackId="1" stroke="hsl(var(--warning))" fill="hsl(var(--warning) / 0.2)" name="Network" />
                  </AreaChart>
                </ResponsiveContainer>
              ) : (
                <div className="h-full flex items-center justify-center text-muted-foreground text-sm">
                  {metrics.isLoading ? "Loading..." : "No data available"}
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Activity Feed */}
      <Card className="border-border/50">
        <CardHeader className="pb-2">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm font-medium text-muted-foreground">Recent Activity</CardTitle>
            <Badge variant="outline" className="text-xs text-success border-success/30">
              <Globe className="h-3 w-3 mr-1" /> Live
            </Badge>
          </div>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow className="border-border/50 hover:bg-transparent">
                <TableHead className="text-xs text-muted-foreground">Type</TableHead>
                <TableHead className="text-xs text-muted-foreground">Event</TableHead>
                <TableHead className="text-xs text-muted-foreground">Category</TableHead>
                <TableHead className="text-xs text-muted-foreground">Status</TableHead>
                <TableHead className="text-xs text-muted-foreground text-right">Time</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {activity.data && Array.isArray(activity.data) ? (
                activity.data.slice(0, 20).map((item: any, i: number) => {
                  const Icon = activityIcons[item.type] || Activity;
                  return (
                    <TableRow key={item.id ?? i} className="border-border/30">
                      <TableCell><Icon className={`h-4 w-4 ${activityColors[item.type] || "text-muted-foreground"}`} /></TableCell>
                      <TableCell className="text-sm text-foreground">{item.event}</TableCell>
                      <TableCell><Badge variant="outline" className="text-[10px]">{item.category ?? item.type}</Badge></TableCell>
                      <TableCell>
                        {item.status && <Badge variant="outline" className={`text-[10px] ${item.status === "success" ? "text-success" : item.status === "error" ? "text-destructive" : "text-warning"}`}>{item.status}</Badge>}
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground text-right">{item.time}</TableCell>
                    </TableRow>
                  );
                })
              ) : (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-sm text-muted-foreground py-8">
                    {activity.isLoading ? "Loading activity..." : "No recent activity"}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}
