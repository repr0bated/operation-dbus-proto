import { PageHeader } from "@/components/shell/PageHeader";
import { useAgents } from "@/hooks/useApi";
import { StatusBadge } from "@/components/shell/StatusBadge";
import { Badge } from "@/components/ui/badge";
import { JsonTree } from "@/components/json/JsonTree";
import { Bot, ChevronRight } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";
import type { AgentInfo } from "@/api/types";

export default function AgentsPage() {
  const { data: agents, isLoading } = useAgents();
  const [selected, setSelected] = useState<AgentInfo | null>(null);

  return (
    <div className="flex h-full">
      <div className="w-72 border-r flex flex-col shrink-0">
        <div className="px-3 py-2 border-b text-xs font-semibold">Agents</div>
        <div className="flex-1 overflow-auto">
          {isLoading ? (
            <div className="p-4 text-xs text-muted-foreground">Loading...</div>
          ) : agents?.length === 0 ? (
            <div className="p-4 text-xs text-muted-foreground">No agents registered</div>
          ) : (
            agents?.map((agent) => (
              <button
                key={agent.id}
                onClick={() => setSelected(agent)}
                className={cn(
                  "w-full text-left px-3 py-2 border-b border-border/50 hover:bg-accent/50",
                  selected?.id === agent.id && "bg-accent"
                )}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Bot className="h-3.5 w-3.5 text-muted-foreground" />
                    <span className="text-xs font-medium">{agent.name}</span>
                  </div>
                  <StatusBadge status={agent.status} />
                </div>
                <p className="text-[10px] text-muted-foreground truncate mt-0.5">{agent.description}</p>
              </button>
            ))
          )}
        </div>
      </div>

      <div className="flex-1 overflow-auto">
        {selected ? (
          <>
            <PageHeader title={selected.name} description={selected.description}>
              <StatusBadge status={selected.status} />
            </PageHeader>
            <div className="p-4 space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <div className="rounded border p-3 space-y-1">
                  <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">Model</div>
                  <div className="font-mono text-sm">{selected.model}</div>
                </div>
                <div className="rounded border p-3 space-y-1">
                  <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">Sessions</div>
                  <div className="font-mono text-sm">{selected.sessions}</div>
                </div>
              </div>

              <div className="space-y-1">
                <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">Capabilities</div>
                <div className="flex flex-wrap gap-1">
                  {selected.capabilities.map((c) => <Badge key={c} variant="outline" className="text-[10px]">{c}</Badge>)}
                </div>
              </div>

              <div className="space-y-1">
                <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">Tools ({selected.tools.length})</div>
                <div className="flex flex-wrap gap-1">
                  {selected.tools.map((t) => <Badge key={t} variant="secondary" className="text-[10px] font-mono">{t}</Badge>)}
                </div>
              </div>
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center h-full text-xs text-muted-foreground">Select an agent</div>
        )}
      </div>
    </div>
  );
}
