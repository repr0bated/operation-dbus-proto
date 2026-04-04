import { PageHeader } from "@/components/shell/PageHeader";
import { useQuery } from "@tanstack/react-query";
import { supabase } from "@/integrations/supabase/client";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Search, Copy, Check } from "lucide-react";
import { useState, useCallback } from "react";
import { cn } from "@/lib/utils";

const levelColors: Record<string, string> = {
  error: "bg-destructive/20 text-destructive border-destructive/30",
  warn: "bg-warning/20 text-warning border-warning/30",
  info: "bg-info/20 text-info border-info/30",
  debug: "bg-muted text-muted-foreground border-border",
  trace: "bg-muted text-muted-foreground border-border",
};

export default function LogsPage() {
  const [search, setSearch] = useState("");
  const [level, setLevel] = useState("all");

  const { data: logs, isLoading } = useQuery({
    queryKey: ["logs-page", level],
    queryFn: async () => {
      let q = supabase.from("logs").select("*, nodes(hostname)").order("timestamp", { ascending: false }).limit(300);
      if (level !== "all") q = q.eq("level", level);
      const { data, error } = await q;
      if (error) throw error;
      return data;
    },
  });

  const filtered = logs?.filter((l) =>
    l.message.toLowerCase().includes(search.toLowerCase()) ||
    (l.source?.toLowerCase().includes(search.toLowerCase()) ?? false)
  );

  const [copiedId, setCopiedId] = useState<string | null>(null);
  const copyLine = useCallback((id: string, msg: string) => {
    navigator.clipboard.writeText(msg);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1200);
  }, []);

  return (
    <div className="flex flex-col h-full">
      <PageHeader title="Logs" description="Live system and application logs">
        <div className="flex gap-2">
          <Select value={level} onValueChange={setLevel}>
            <SelectTrigger className="w-28 h-7 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All</SelectItem>
              <SelectItem value="error">Error</SelectItem>
              <SelectItem value="warn">Warning</SelectItem>
              <SelectItem value="info">Info</SelectItem>
              <SelectItem value="debug">Debug</SelectItem>
            </SelectContent>
          </Select>
          <div className="relative w-56">
            <Search className="absolute left-2 top-2 h-3.5 w-3.5 text-muted-foreground" />
            <Input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Search logs..." className="pl-7 h-7 text-xs" />
          </div>
        </div>
      </PageHeader>
      <div className="flex-1 overflow-auto font-mono text-xs">
        {isLoading ? (
          <div className="p-4 text-muted-foreground text-center">Loading logs...</div>
        ) : filtered?.length === 0 ? (
          <div className="p-4 text-muted-foreground text-center">No logs found</div>
        ) : (
          filtered?.map((log) => (
            <div
              key={log.id}
              className="flex items-start gap-2 px-3 py-1 border-b border-border/30 hover:bg-accent/30 group"
            >
              <span className="text-muted-foreground shrink-0 w-[72px] tabular-nums">
                {new Date(log.timestamp).toLocaleTimeString()}
              </span>
              <span className={cn("inline-flex items-center rounded-sm border px-1 py-0 text-[9px] font-medium shrink-0 w-10 justify-center", levelColors[log.level] || levelColors.debug)}>
                {log.level}
              </span>
              <span className="text-muted-foreground shrink-0 w-20 truncate">{log.source ?? "—"}</span>
              <span className="text-muted-foreground shrink-0 w-16 truncate">{(log as any).nodes?.hostname ?? ""}</span>
              <span className="flex-1 whitespace-pre-wrap break-all">{log.message}</span>
              <button
                onClick={() => copyLine(log.id, log.message)}
                className="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-foreground shrink-0"
              >
                {copiedId === log.id ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
