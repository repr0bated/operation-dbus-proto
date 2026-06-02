import { useState, useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { Search, Download, Pause, Play, Filter } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { getDashboardProjections } from "@/lib/api";

interface LogEntry {
  id: number;
  timestamp: string;
  level: "info" | "warn" | "error" | "debug";
  source: string;
  message: string;
}

const levelColors: Record<string, string> = {
  info: "text-blue-600 dark:text-blue-400 bg-blue-500/10",
  warn: "text-yellow-600 dark:text-yellow-400 bg-yellow-500/10",
  error: "text-red-600 dark:text-red-400 bg-red-500/10",
  debug: "text-muted-foreground bg-muted",
};

function projectionsToLogs(data: any, prev: LogEntry[]): LogEntry[] {
  if (!data || typeof data !== "object") return prev;

  const now = new Date();
  const ts =
    now.toTimeString().split(" ")[0] +
    "." +
    String(now.getMilliseconds()).padStart(3, "0");
  const baseId = prev.length > 0 ? prev[prev.length - 1].id : 0;
  const entries: LogEntry[] = [];

  // Memory log
  const mem = data["system.memory"];
  if (mem) {
    const used =
      mem.memtotal && mem.memfree
        ? Math.round(
            ((parseInt(mem.memtotal) - parseInt(mem.memfree)) /
              parseInt(mem.memtotal)) *
              100,
          )
        : "—";
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.memory",
      message: `memtotal=${mem.memtotal ?? "?"} memfree=${mem.memfree ?? "?"} used=${used}%`,
    });
  }

  // Load log
  const load = data["system.load"];
  if (load) {
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: load.load1 > load.count ? "warn" : "info",
      source: "system.load",
      message: `load1=${load.load1 ?? "?"} load5=${load.load5 ?? "?"} procs_running=${load.procs_running ?? "?"} procs_total=${load.procs_total ?? "?"}`,
    });
  }

  // CPU log
  const cpu = data["system.cpu"];
  if (cpu) {
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.cpu",
      message: `cores=${cpu.count ?? "?"} model=${cpu.cpus?.[0]?.model_name ?? "?"}`,
    });
  }

  // Network log
  const net = data["system.network"];
  if (net && Array.isArray(net.interfaces)) {
    const ifaces = net.interfaces
      .map((i: any) => i.name)
      .filter(Boolean)
      .join(", ");
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.network",
      message: `interfaces=[${ifaces}]`,
    });
  }

  // Processes log
  const procs = data["system.processes"];
  if (procs) {
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.processes",
      message: `total=${procs.processes ?? "?"} running=${procs.procs_running ?? "?"} blocked=${procs.procs_blocked ?? "?"}`,
    });
  }

  // Filesystems log
  const fs = data["system.filesystems"];
  if (fs && Array.isArray(fs)) {
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.filesystems",
      message: `mounts=${fs.length}`,
    });
  }

  // Keep only the last 200 entries to avoid unbounded growth
  const combined = [...prev, ...entries];
  if (combined.length > 200) return combined.slice(combined.length - 200);
  return combined;
}

export default function LogsPage() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [level, setLevel] = useState("all");
  const [paused, setPaused] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const projections = useQuery({
    queryKey: ["dashboardProjections"],
    queryFn: getDashboardProjections,
    refetchInterval: paused ? false : 5000,
  });

  // Append new projection data as log entries when it arrives
  useEffect(() => {
    if (projections.data && !paused) {
      setLogs((prev) => projectionsToLogs(projections.data, prev));
    }
  }, [projections.data, paused]);

  // Auto-scroll to bottom
  useEffect(() => {
    if (containerRef.current && !paused) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [logs, paused]);

  const filtered = logs.filter((log) => {
    if (level !== "all" && log.level !== level) return false;
    if (
      filter &&
      !log.message.toLowerCase().includes(filter.toLowerCase()) &&
      !log.source.toLowerCase().includes(filter.toLowerCase())
    )
      return false;
    return true;
  });

  const exportLogs = () => {
    const text = filtered
      .map(
        (l) =>
          `${l.timestamp} [${l.level.toUpperCase()}] ${l.source}: ${l.message}`,
      )
      .join("\n");
    const blob = new Blob([text], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "dbus-projection-logs.txt";
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">
            Logs
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Live tail of D-Bus projection tree (
            {projections.data ? Object.keys(projections.data).length : 0}{" "}
            schemas)
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            className="gap-1.5"
            onClick={() => setPaused(!paused)}
          >
            {paused ? (
              <Play className="h-4 w-4" />
            ) : (
              <Pause className="h-4 w-4" />
            )}
            {paused ? "Resume" : "Pause"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="gap-1.5"
            onClick={exportLogs}
          >
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
          {filtered.length === 0 && projections.isLoading && (
            <div className="px-3 py-8 text-center text-muted-foreground">
              Loading projections...
            </div>
          )}
          {filtered.length === 0 && !projections.isLoading && (
            <div className="px-3 py-8 text-center text-muted-foreground">
              No projection data yet. Waiting for D-Bus tree...
            </div>
          )}
          {filtered.map((log) => (
            <div
              key={log.id}
              className="flex items-start gap-2 px-3 py-1.5 hover:bg-accent/50 transition-colors rounded"
            >
              <span className="text-muted-foreground shrink-0 w-24">
                {log.timestamp}
              </span>
              <Badge
                className={`text-[10px] h-5 shrink-0 w-12 justify-center ${levelColors[log.level]}`}
                variant="secondary"
              >
                {log.level.toUpperCase()}
              </Badge>
              <span className="text-primary shrink-0 w-28 truncate">
                {log.source}
              </span>
              <span className="text-foreground break-all">{log.message}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>
          {filtered.length} entries shown ({logs.length} total)
        </span>
        <span className="flex items-center gap-1">
          <span
            className={`h-1.5 w-1.5 rounded-full ${paused ? "bg-yellow-500" : projections.isFetching ? "bg-green-500 animate-pulse" : "bg-green-500"}`}
          />
          {paused ? "Paused" : projections.isFetching ? "Fetching..." : "Live"}
        </span>
      </div>
    </div>
  );
}
