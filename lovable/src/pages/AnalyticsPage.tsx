import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  BarChart, Bar, LineChart, Line, PieChart, Pie, Cell, AreaChart, Area,
  XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend,
} from "recharts";
import {
  getAnalyticsVpnTraffic, getAnalyticsVpnUsers, getAnalyticsVpnPeakHours,
  getAnalyticsChatMessages, getAnalyticsChatLengths, getAnalyticsChatIntents,
  getAnalyticsMailVolume, getAnalyticsMailDelivery, getAnalyticsMailDomains,
} from "@/lib/api";

const COLORS = [
  "hsl(var(--primary))", "hsl(var(--success))", "hsl(var(--warning))",
  "hsl(var(--destructive))", "hsl(var(--info))",
];

const tooltipStyle = {
  background: "hsl(var(--card))",
  border: "1px solid hsl(var(--border))",
  borderRadius: "8px",
  fontSize: 12,
};

function ChartCard({ title, children, className }: { title: string; children: React.ReactNode; className?: string }) {
  return (
    <Card className={`border-border/50 ${className ?? ""}`}>
      <CardHeader className="pb-2"><CardTitle className="text-sm font-medium text-muted-foreground">{title}</CardTitle></CardHeader>
      <CardContent><div className="h-56">{children}</div></CardContent>
    </Card>
  );
}

function EmptyChart({ loading }: { loading: boolean }) {
  return <div className="h-full flex items-center justify-center text-muted-foreground text-sm">{loading ? "Loading..." : "No data"}</div>;
}

export default function AnalyticsPage() {
  const [range, setRange] = useState("7d");
  const toArr = (d: any) => Array.isArray(d) ? d : [];

  const vpnTraffic = useQuery({ queryKey: ["a-vpn-traffic", range], queryFn: () => getAnalyticsVpnTraffic(range) });
  const vpnUsers = useQuery({ queryKey: ["a-vpn-users"], queryFn: getAnalyticsVpnUsers });
  const vpnPeak = useQuery({ queryKey: ["a-vpn-peak"], queryFn: getAnalyticsVpnPeakHours });
  const chatMsgs = useQuery({ queryKey: ["a-chat-msgs", range], queryFn: () => getAnalyticsChatMessages(range) });
  const chatLengths = useQuery({ queryKey: ["a-chat-lengths"], queryFn: getAnalyticsChatLengths });
  const chatIntents = useQuery({ queryKey: ["a-chat-intents"], queryFn: getAnalyticsChatIntents });
  const mailVolume = useQuery({ queryKey: ["a-mail-vol", range], queryFn: () => getAnalyticsMailVolume(range) });
  const mailDelivery = useQuery({ queryKey: ["a-mail-del", range], queryFn: () => getAnalyticsMailDelivery(range) });
  const mailDomains = useQuery({ queryKey: ["a-mail-domains"], queryFn: getAnalyticsMailDomains });

  return (
    <div className="space-y-6 animate-slide-in">
      <div className="flex items-center justify-between">
        <div><h1 className="text-2xl font-bold text-foreground">Analytics</h1><p className="text-sm text-muted-foreground mt-1">Usage metrics & trends</p></div>
        <Select value={range} onValueChange={setRange}>
          <SelectTrigger className="w-36 bg-secondary border-none text-sm"><SelectValue /></SelectTrigger>
          <SelectContent><SelectItem value="24h">Today</SelectItem><SelectItem value="7d">Last 7 days</SelectItem><SelectItem value="30d">Last 30 days</SelectItem></SelectContent>
        </Select>
      </div>

      {/* VPN Analytics */}
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">VPN Analytics</h2>
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <ChartCard title="Data Transfer Over Time">
          {toArr(vpnTraffic.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={toArr(vpnTraffic.data)}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="day" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <Tooltip contentStyle={tooltipStyle} /><Legend />
                <Area type="monotone" dataKey="upload" stackId="1" stroke="hsl(var(--primary))" fill="hsl(var(--primary) / 0.2)" />
                <Area type="monotone" dataKey="download" stackId="1" stroke="hsl(var(--info))" fill="hsl(var(--info) / 0.2)" />
              </AreaChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={vpnTraffic.isLoading} />}
        </ChartCard>

        <ChartCard title="Users by Status">
          {toArr(vpnUsers.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie data={toArr(vpnUsers.data)} dataKey="count" nameKey="status" cx="50%" cy="50%" outerRadius={70} label={({ name, percent }) => `${name} ${(percent * 100).toFixed(0)}%`} labelLine={false}>
                  {toArr(vpnUsers.data).map((_: any, i: number) => <Cell key={i} fill={COLORS[i % COLORS.length]} />)}
                </Pie>
                <Tooltip contentStyle={tooltipStyle} />
              </PieChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={vpnUsers.isLoading} />}
        </ChartCard>

        <ChartCard title="Peak Usage Hours">
          {toArr(vpnPeak.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={toArr(vpnPeak.data)}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="hour" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <Tooltip contentStyle={tooltipStyle} />
                <Bar dataKey="connections" fill="hsl(var(--primary))" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={vpnPeak.isLoading} />}
        </ChartCard>
      </div>

      {/* Chat Analytics */}
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Chat & Agent Analytics</h2>
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <ChartCard title="Messages Per Day">
          {toArr(chatMsgs.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={toArr(chatMsgs.data)}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="day" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <Tooltip contentStyle={tooltipStyle} /><Legend />
                <Line type="monotone" dataKey="messages" stroke="hsl(var(--primary))" strokeWidth={2} dot={false} />
                <Line type="monotone" dataKey="sessions" stroke="hsl(var(--success))" strokeWidth={2} dot={false} />
              </LineChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={chatMsgs.isLoading} />}
        </ChartCard>

        <ChartCard title="Conversation Length Distribution">
          {toArr(chatLengths.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={toArr(chatLengths.data)}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="range" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <Tooltip contentStyle={tooltipStyle} />
                <Bar dataKey="count" fill="hsl(var(--success))" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={chatLengths.isLoading} />}
        </ChartCard>

        <ChartCard title="Popular Intent Categories">
          {toArr(chatIntents.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={toArr(chatIntents.data)} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis type="number" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis type="category" dataKey="intent" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} width={80} />
                <Tooltip contentStyle={tooltipStyle} />
                <Bar dataKey="count" fill="hsl(var(--warning))" radius={[0, 4, 4, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={chatIntents.isLoading} />}
        </ChartCard>
      </div>

      {/* Mail Analytics */}
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Mail Analytics</h2>
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <ChartCard title="Email Volume">
          {toArr(mailVolume.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={toArr(mailVolume.data)}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="day" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <Tooltip contentStyle={tooltipStyle} /><Legend />
                <Bar dataKey="sent" fill="hsl(var(--warning))" radius={[4, 4, 0, 0]} />
                <Bar dataKey="received" fill="hsl(var(--success))" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={mailVolume.isLoading} />}
        </ChartCard>

        <ChartCard title="Delivery Success Rate">
          {toArr(mailDelivery.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={toArr(mailDelivery.data)}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="day" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} domain={[0, 100]} />
                <Tooltip contentStyle={tooltipStyle} />
                <Line type="monotone" dataKey="rate" stroke="hsl(var(--success))" strokeWidth={2} dot={false} />
              </LineChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={mailDelivery.isLoading} />}
        </ChartCard>

        <ChartCard title="Top Recipient Domains">
          {toArr(mailDomains.data).length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={toArr(mailDomains.data)} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis type="number" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis type="category" dataKey="domain" tick={{ fontSize: 10, fill: "hsl(var(--muted-foreground))" }} width={100} />
                <Tooltip contentStyle={tooltipStyle} />
                <Bar dataKey="count" fill="hsl(var(--info))" radius={[0, 4, 4, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : <EmptyChart loading={mailDomains.isLoading} />}
        </ChartCard>
      </div>
    </div>
  );
}
