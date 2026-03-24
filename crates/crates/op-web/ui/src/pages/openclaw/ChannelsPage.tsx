import { MessageSquare, QrCode, Settings, CheckCircle, XCircle, AlertTriangle, RefreshCw } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

interface Channel {
  id: string;
  name: string;
  type: "whatsapp" | "telegram" | "discord" | "slack" | "mattermost";
  status: "connected" | "disconnected" | "error";
  messages24h: number;
  lastMessage?: string;
}

const channels: Channel[] = [
  { id: "1", name: "Personal WhatsApp", type: "whatsapp", status: "connected", messages24h: 23, lastMessage: "2m ago" },
  { id: "2", name: "Dev Team", type: "telegram", status: "connected", messages24h: 156, lastMessage: "Just now" },
  { id: "3", name: "Server Bot", type: "discord", status: "connected", messages24h: 89, lastMessage: "5m ago" },
  { id: "4", name: "Workspace", type: "slack", status: "error", messages24h: 0, lastMessage: "Token expired" },
  { id: "5", name: "Internal", type: "mattermost", status: "disconnected", messages24h: 0 },
];

const statusIcon: Record<string, React.ReactNode> = {
  connected: <CheckCircle className="h-4 w-4 text-green-500" />,
  disconnected: <XCircle className="h-4 w-4 text-muted-foreground" />,
  error: <AlertTriangle className="h-4 w-4 text-yellow-500" />,
};

const typeColors: Record<string, string> = {
  whatsapp: "bg-green-500/10 text-green-600 dark:text-green-400",
  telegram: "bg-blue-500/10 text-blue-600 dark:text-blue-400",
  discord: "bg-indigo-500/10 text-indigo-600 dark:text-indigo-400",
  slack: "bg-purple-500/10 text-purple-600 dark:text-purple-400",
  mattermost: "bg-cyan-500/10 text-cyan-600 dark:text-cyan-400",
};

export default function ChannelsPage() {
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Channels</h1>
          <p className="text-sm text-muted-foreground mt-1">
            WhatsApp, Telegram, Discord, Slack & plugin channels
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" className="gap-1.5">
            <QrCode className="h-4 w-4" /> QR Login
          </Button>
          <Button size="sm" className="gap-1.5">
            <MessageSquare className="h-4 w-4" /> Add Channel
          </Button>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        {channels.map((channel) => (
          <Card key={channel.id} className="bg-card border-border hover:border-primary/20 transition-colors">
            <CardHeader className="pb-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  {statusIcon[channel.status]}
                  <CardTitle className="text-sm font-semibold">{channel.name}</CardTitle>
                </div>
                <Badge className={`text-[10px] ${typeColors[channel.type]}`} variant="secondary">
                  {channel.type}
                </Badge>
              </div>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                <div className="flex justify-between text-xs">
                  <span className="text-muted-foreground">Messages (24h)</span>
                  <span className="text-foreground font-medium">{channel.messages24h}</span>
                </div>
                <div className="flex justify-between text-xs">
                  <span className="text-muted-foreground">Last activity</span>
                  <span className="text-foreground">{channel.lastMessage || "Never"}</span>
                </div>
                <div className="flex gap-2 pt-2">
                  <Button variant="outline" size="sm" className="flex-1 h-7 text-xs gap-1">
                    <Settings className="h-3 w-3" /> Config
                  </Button>
                  <Button variant="outline" size="sm" className="h-7 text-xs gap-1">
                    <RefreshCw className="h-3 w-3" />
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
