import { useState } from "react";
import { Clock, MoreVertical, MessageSquare, Eye, Brain, Trash2 } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

interface Session {
  id: string;
  key: string;
  model: string;
  status: "active" | "idle" | "disconnected";
  messages: number;
  thinking: "auto" | "on" | "off";
  verbose: boolean;
  lastActive: string;
  uptime: string;
}

const sessions: Session[] = [
  { id: "1", key: "default", model: "claude-4-opus", status: "active", messages: 142, thinking: "auto", verbose: false, lastActive: "Just now", uptime: "4d 12h" },
  { id: "2", key: "coding", model: "claude-4-opus", status: "active", messages: 89, thinking: "on", verbose: true, lastActive: "2m ago", uptime: "2d 6h" },
  { id: "3", key: "research", model: "gpt-5", status: "idle", messages: 34, thinking: "auto", verbose: false, lastActive: "1h ago", uptime: "12h" },
  { id: "4", key: "automation", model: "claude-4-sonnet", status: "disconnected", messages: 567, thinking: "off", verbose: false, lastActive: "2d ago", uptime: "—" },
];

const statusColor: Record<string, string> = {
  active: "bg-green-500",
  idle: "bg-yellow-500",
  disconnected: "bg-muted-foreground/40",
};

export default function SessionsPage() {
  const [sessionList, setSessionList] = useState(sessions);

  const toggleVerbose = (id: string) => {
    setSessionList((prev) =>
      prev.map((s) => (s.id === id ? { ...s, verbose: !s.verbose } : s))
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Sessions</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Manage active sessions with thinking and verbose overrides
          </p>
        </div>
        <Badge variant="secondary" className="text-xs">
          {sessionList.filter((s) => s.status === "active").length} active
        </Badge>
      </div>

      <div className="grid gap-3">
        {sessionList.map((session) => (
          <Card key={session.id} className="bg-card border-border">
            <CardContent className="p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <span className={`h-2 w-2 rounded-full ${statusColor[session.status]}`} />
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="font-semibold text-sm text-foreground">{session.key}</span>
                      <Badge variant="outline" className="text-[10px] font-mono h-5">
                        {session.model}
                      </Badge>
                    </div>
                    <div className="flex items-center gap-3 mt-1 text-xs text-muted-foreground">
                      <span className="flex items-center gap-1">
                        <MessageSquare className="h-3 w-3" />
                        {session.messages} msgs
                      </span>
                      <span className="flex items-center gap-1">
                        <Clock className="h-3 w-3" />
                        {session.lastActive}
                      </span>
                      <span>Uptime: {session.uptime}</span>
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-4">
                  <div className="flex items-center gap-2 text-xs">
                    <Brain className="h-3.5 w-3.5 text-muted-foreground" />
                    <span className="text-muted-foreground">Thinking:</span>
                    <Badge variant="secondary" className="text-[10px] h-5">{session.thinking}</Badge>
                  </div>

                  <div className="flex items-center gap-2 text-xs">
                    <Eye className="h-3.5 w-3.5 text-muted-foreground" />
                    <span className="text-muted-foreground">Verbose</span>
                    <Switch
                      checked={session.verbose}
                      onCheckedChange={() => toggleVerbose(session.id)}
                      className="scale-75"
                    />
                  </div>

                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="ghost" size="icon" className="h-8 w-8">
                        <MoreVertical className="h-4 w-4" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem className="text-sm">Open in Chat</DropdownMenuItem>
                      <DropdownMenuItem className="text-sm">Clear History</DropdownMenuItem>
                      <DropdownMenuItem className="text-sm text-destructive">
                        <Trash2 className="h-3.5 w-3.5 mr-2" /> End Session
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
