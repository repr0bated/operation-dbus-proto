import { useState, useEffect, useRef } from "react";
import { Search, Download, Pause, Play, Filter } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

interface LogEntry {
  id: number;
  timestamp: string;
  level: "info" | "warn" | "error" | "debug";
  source: string;
  message: string;
}

const generateLogs = (): LogEntry[] => [
  { id: 1, timestamp: "14:32:05.123", level: "info", source: "gateway", message: "chat.complete runId=r_abc123 tokens_in=1247 tokens_out=892 latency=3.2s" },
  { id: 2, timestamp: "14:32:03.456", level: "debug", source: "skill:web-search", message: "query='server health monitoring' results=3 cached=false" },
  { id: 3, timestamp: "14:32:01.789", level: "info", source: "gateway", message: "chat.send session=default model=claude-4-opus runId=r_abc123" },
  { id: 4, timestamp: "14:30:00.001", level: "info", source: "cron", message: "job='Check Server Health' started schedule='*/15 * * * *'" },
  { id: 5, timestamp: "14:30:02.334", level: "info", source: "cron", message: "job='Check Server Health' completed status=success duration=2.3s" },
  { id: 6, timestamp: "14:15:02.100", level: "info", source: "session", message: "session.create key=coding model=claude-4-opus" },
  { id: 7, timestamp: "14:10:00.001", level: "info", source: "cron", message: "job='Check Server Health' started" },
  { id: 8, timestamp: "14:10:01.890", level: "info", source: "cron", message: "job='Check Server Health' completed status=success duration=1.9s" },
  { id: 9, timestamp: "14:05:12.456", level: "warn", source: "gateway", message: "ws connection from 192.168.1.50 — device not paired, sending 1008" },
  { id: 10, timestamp: "14:00:00.000", level: "info", source: "gateway", message: "health check ok sessions=3 memory=128MB ws=2" },
  { id: 11, timestamp: "13:55:33.221", level: "error", source: "skill:email", message: "IMAP connection failed: ETIMEDOUT after 30s host=imap.gmail.com:993" },
  { id: 12, timestamp: "13:50:00.001", level: "info", source: "cron", message: "job='News Digest' started" },
  { id: 13, timestamp: "13:50:15.678", level: "error", source: "cron", message: "job='News Digest' failed: provider rate limit exceeded (429)" },
  { id: 14, timestamp: "13:45:00.001", level: "info", source: "cron", message: "job='Check Server Health' completed status=success" },
  { id: 15, timestamp: "13:30:12.345", level: "debug", source: "gateway", message: "config.get client=control-ui hash=a3f8b2c1" },
];

const levelColors: Record<string, string> = {
  info: "text-blue-600 dark:text-blue-400 bg-blue-500/10",
  warn: "text-yellow-600 dark:text-yellow-400 bg-yellow-500/10",
  error: "text-red-600 dark:text-red-400 bg-red-500/10",
  debug: "text-muted-foreground bg-muted",
};

export default function LogsPage() {
  const [logs] = useState(generateLogs);
  const [filter, setFilter] = useState("");
  const [level, setLevel] = useState("all");
  const [paused, setPaused] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const filtered = logs.filter((log) => {
    if (level !== "all" && log.level !== level) return false;
    if (filter && !log.message.toLowerCase().includes(filter.toLowerCase()) && !log.source.toLowerCase().includes(filter.toLowerCase())) return false;
    return true;
  });

  const exportLogs = () => {
    const text = filtered.map((l) => `${l.timestamp} [${l.level.toUpperCase()}] ${l.source}: ${l.message}`).join("\n");
    const blob = new Blob([text], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "openclaw-logs.txt";
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Logs</h1>
          <p className="text-sm text-muted-foreground mt-1">Live tail of gateway file logs</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setPaused(!paused)}>
            {paused ? <Play className="h-4 w-4" /> : <Pause className="h-4 w-4" />}
            {paused ? "Resume" : "Pause"}
          </Button>
          <Button variant="outline" size="sm" className="gap-1.5" onClick={exportLogs}>
            <Download className="h-4 w-4" /> Export
          </Button>
        </div>
      </div>

      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Filter logs..."
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="pl-9 text-sm"
          />
        </div>
        <Select value={level} onValueChange={setLevel}>
          <SelectTrigger className="w-28 text-sm">
            <Filter className="h-3.5 w-3.5 mr-1" />
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All</SelectItem>
            <SelectItem value="info">Info</SelectItem>
            <SelectItem value="warn">Warn</SelectItem>
            <SelectItem value="error">Error</SelectItem>
            <SelectItem value="debug">Debug</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div
        ref={containerRef}
        className="bg-card border border-border rounded-lg overflow-auto max-h-[calc(100vh-280px)] font-mono text-xs"
      >
        <div className="p-1">
          {filtered.map((log) => (
            <div
              key={log.id}
              className="flex items-start gap-2 px-3 py-1.5 hover:bg-accent/50 transition-colors rounded"
            >
              <span className="text-muted-foreground shrink-0 w-24">{log.timestamp}</span>
              <Badge className={`text-[10px] h-5 shrink-0 w-12 justify-center ${levelColors[log.level]}`} variant="secondary">
                {log.level.toUpperCase()}
              </Badge>
              <span className="text-primary shrink-0 w-28 truncate">{log.source}</span>
              <span className="text-foreground break-all">{log.message}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>{filtered.length} entries shown</span>
        <span className="flex items-center gap-1">
          <span className={`h-1.5 w-1.5 rounded-full ${paused ? "bg-yellow-500" : "bg-green-500 animate-pulse"}`} />
          {paused ? "Paused" : "Live"}
        </span>
      </div>
    </div>
  );
}
