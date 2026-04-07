/**
 * Orchestration View — monitors live multi-agent execution, coordination strategies,
 * and task results with a React Flow execution graph.
 */
import { useState, useCallback, useMemo, useEffect } from "react";
import {
  ReactFlow, Background, Controls,
  useNodesState, useEdgesState,
  type Node, type Edge, ReactFlowProvider,
  Handle, Position, type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { PageHeader, Card, StatusDot } from "@/components/shell/Primitives";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { useEventStore } from "@/stores/event-store";
import { cn } from "@/lib/utils";
import {
  Bot, Activity, ChevronRight, CheckCircle, XCircle, Clock, Play,
  RotateCcw, Zap, Network,
} from "lucide-react";

// ── Types ───────────────────────────────────────────────
type AgentStatus = "idle" | "busy" | "error" | "offline";
type Strategy = "pipeline" | "parallel" | "race_first_success" | "sequential";

interface AgentInfo {
  id: string;
  name: string;
  status: AgentStatus;
  activeTask: string | null;
  model: string;
  emoji?: string;
}

interface TaskResult {
  id: string;
  taskId: string;
  agent: string;
  success: boolean;
  durationMs: number;
  result: unknown;
  timestamp: number;
}

// ── Agent Status Colors ──────────────────────────────────
const statusVariant: Record<AgentStatus, "ok" | "warn" | "error" | "offline"> = {
  idle: "ok", busy: "warn", error: "error", offline: "offline",
};

const statusBadge: Record<AgentStatus, string> = {
  idle: "border-ok/20 bg-ok/10 text-ok",
  busy: "border-warn/20 bg-warn/10 text-warn",
  error: "border-danger/20 bg-danger/10 text-danger",
  offline: "border-border text-muted-foreground",
};

// ── Custom Node ──────────────────────────────────────────
interface AgentNodeData { agent: AgentInfo; [key: string]: unknown; }

function AgentNode({ data }: NodeProps) {
  const d = data as unknown as AgentNodeData;
  const agent = d.agent;
  const isBusy = agent.status === "busy";

  return (
    <div className={cn(
      "rounded-lg border bg-card shadow-lg min-w-[180px] overflow-hidden transition-all",
      isBusy ? "border-warn/40 shadow-[0_0_12px_hsl(var(--warn)/0.15)]" : "border-border",
      agent.status === "error" && "border-danger/40 shadow-[0_0_12px_hsl(var(--danger)/0.15)]",
    )}>
      <Handle type="target" position={Position.Left} className="!bg-primary !border-primary/50 !w-2.5 !h-2.5" />
      <div className="px-3 py-2 flex items-center gap-2 border-b border-border/50">
        <span className="text-base">{agent.emoji}</span>
        <div className="flex-1 min-w-0">
          <div className="text-xs font-semibold text-foreground truncate">{agent.name}</div>
          <div className="text-[10px] text-muted-foreground font-mono">{agent.model}</div>
        </div>
        <StatusDot status={statusVariant[agent.status]} />
      </div>
      <div className="px-3 py-2">
        <Badge variant="outline" className={cn("text-[9px]", statusBadge[agent.status])}>{agent.status}</Badge>
        {agent.activeTask && (
          <div className="mt-1.5 text-[10px] text-muted-foreground font-mono truncate max-w-[160px]">{agent.activeTask}</div>
        )}
      </div>
      {isBusy && (
        <div className="h-0.5 bg-warn/20 overflow-hidden">
          <div className="h-full w-1/3 bg-warn rounded-full animate-[shimmer_1.5s_ease-in-out_infinite]" />
        </div>
      )}
      <Handle type="source" position={Position.Right} className="!bg-ok !border-ok/50 !w-2.5 !h-2.5" />
    </div>
  );
}

const nodeTypes = { agentNode: AgentNode };

// ── Build Graph from Strategy ────────────────────────────
function buildGraph(agents: AgentInfo[], strategy: Strategy): { nodes: Node[]; edges: Edge[] } {
  const busyAgents = agents.filter((a) => a.status !== "offline");
  const nodes: Node[] = [];
  const edges: Edge[] = [];

  nodes.push({
    id: "coordinator", type: "agentNode", position: { x: 50, y: 150 },
    data: {
      agent: { id: "coordinator", name: "Coordinator", status: "busy" as AgentStatus, activeTask: `Strategy: ${strategy}`, model: "orchestrator", emoji: "🎯" },
    } satisfies AgentNodeData as any,
  });

  if (strategy === "pipeline" || strategy === "sequential") {
    busyAgents.forEach((agent, i) => {
      nodes.push({ id: agent.id, type: "agentNode", position: { x: 320 + i * 260, y: 150 }, data: { agent } satisfies AgentNodeData as any });
      const source = i === 0 ? "coordinator" : busyAgents[i - 1].id;
      edges.push({ id: `e-${source}-${agent.id}`, source, target: agent.id, animated: agent.status === "busy", style: { stroke: agent.status === "busy" ? "hsl(var(--warn))" : "hsl(var(--border))", strokeWidth: 2 } });
    });
  } else {
    const spacing = 80;
    const totalHeight = (busyAgents.length - 1) * spacing;
    const startY = 150 - totalHeight / 2;
    busyAgents.forEach((agent, i) => {
      nodes.push({ id: agent.id, type: "agentNode", position: { x: 350, y: startY + i * spacing }, data: { agent } satisfies AgentNodeData as any });
      edges.push({ id: `e-coord-${agent.id}`, source: "coordinator", target: agent.id, animated: agent.status === "busy", style: { stroke: agent.status === "busy" ? "hsl(var(--warn))" : "hsl(var(--border))", strokeWidth: 2 } });
    });
  }
  return { nodes, edges };
}

// ── Main Page ────────────────────────────────────────────
function OrchestrationInner() {
  const { latestState } = useEventStore();

  const agents = useMemo(() => {
    const raw = latestState["orchestration.agents"] ?? latestState["orchestration:agents"] ?? latestState["agents.pool"];
    if (Array.isArray(raw)) return raw as AgentInfo[];
    return [] as AgentInfo[];
  }, [latestState]);

  const taskResults = useMemo(() => {
    const raw = latestState["orchestration.tasks"] ?? latestState["orchestration:task_results"] ?? latestState["task_results"];
    if (Array.isArray(raw)) return raw as TaskResult[];
    return [] as TaskResult[];
  }, [latestState]);

  const liveStrategy = latestState["orchestration.strategy"] ?? latestState["orchestration:strategy"];
  const [strategy, setStrategy] = useState<Strategy>((liveStrategy as Strategy) ?? "pipeline");

  useEffect(() => {
    if (liveStrategy && typeof liveStrategy === "string") setStrategy(liveStrategy as Strategy);
  }, [liveStrategy]);

  const graph = useMemo(() => buildGraph(agents, strategy), [agents, strategy]);
  const [nodes, setNodes, onNodesChange] = useNodesState(graph.nodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(graph.edges);
  const [expandedRows, setExpandedRows] = useState<Set<string>>(new Set());

  useEffect(() => {
    const g = buildGraph(agents, strategy);
    setNodes(g.nodes);
    setEdges(g.edges);
  }, [agents, strategy, setNodes, setEdges]);

  const toggleRow = (id: string) => {
    setExpandedRows((prev) => { const next = new Set(prev); next.has(id) ? next.delete(id) : next.add(id); return next; });
  };

  const strategies: Strategy[] = ["pipeline", "parallel", "race_first_success", "sequential"];

  return (
    <div className="space-y-6">
      <PageHeader title="Orchestration" subtitle="Multi-agent execution monitoring and coordination control."
        actions={
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-1 rounded-md border border-border bg-secondary p-0.5">
              {strategies.map((s) => (
                <button key={s} onClick={() => setStrategy(s)}
                  className={cn("px-2 py-1 text-[10px] font-mono rounded transition-colors", strategy === s ? "bg-primary/15 text-primary font-medium" : "text-muted-foreground hover:text-foreground")}>
                  {s.replace("_", " ")}
                </button>
              ))}
            </div>
            <Button size="sm" className="h-7 text-xs gap-1.5"><Play className="h-3 w-3" />Execute</Button>
          </div>
        }
      />

      {/* Agent Pool */}
      {agents.length === 0 ? (
        <div className="text-sm text-muted-foreground text-center py-8">No agents detected. Waiting for live data…</div>
      ) : (
        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3">
          {agents.map((agent) => (
            <Card key={agent.id} className="!p-3">
              <div className="flex items-center gap-2 mb-2">
                <span className="text-lg">{agent.emoji}</span>
                <div className="flex-1 min-w-0">
                  <div className="text-xs font-semibold text-foreground truncate">{agent.name}</div>
                  <div className="text-[10px] text-muted-foreground font-mono">{agent.model}</div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <StatusDot status={statusVariant[agent.status]} />
                <Badge variant="outline" className={cn("text-[9px]", statusBadge[agent.status])}>{agent.status}</Badge>
              </div>
              {agent.activeTask && <div className="mt-2 text-[10px] text-muted-foreground font-mono line-clamp-2">{agent.activeTask}</div>}
            </Card>
          ))}
        </div>
      )}

      {/* Execution Graph */}
      <Card title="Execution Graph" subtitle={`Active strategy: ${strategy.replace("_", " ")}`}
        actions={
          <div className="flex items-center gap-2">
            <Badge variant="outline" className="text-[10px]"><Network className="h-3 w-3 mr-1" />{nodes.length} nodes</Badge>
            <Button size="sm" variant="ghost" className="h-6 text-xs gap-1"><RotateCcw className="h-3 w-3" />Reset</Button>
          </div>
        }
      >
        <div className="h-[320px] rounded-lg border border-border overflow-hidden mt-2 bg-background">
          <ReactFlow nodes={nodes} edges={edges} onNodesChange={onNodesChange} onEdgesChange={onEdgesChange} nodeTypes={nodeTypes} fitView className="[&_.react-flow__background]:!bg-background" proOptions={{ hideAttribution: true }}>
            <Background gap={20} size={1} color="hsl(var(--border))" />
            <Controls className="[&_button]:!bg-card [&_button]:!border-border [&_button]:!text-foreground [&_button:hover]:!bg-muted" />
          </ReactFlow>
        </div>
      </Card>

      {/* Task Ledger */}
      <Card title="Task Ledger" subtitle="Live feed of TaskResult events from the orchestrator."
        actions={<Badge variant="outline" className="text-[10px]">{taskResults.length} results</Badge>}>
        <div className="mt-2 rounded-lg border border-border overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="text-[11px] w-8" />
                <TableHead className="text-[11px]">Task ID</TableHead>
                <TableHead className="text-[11px]">Agent</TableHead>
                <TableHead className="text-[11px]">Status</TableHead>
                <TableHead className="text-[11px] text-right">Duration</TableHead>
                <TableHead className="text-[11px] text-right">Time</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {taskResults.length === 0 && (
                <TableRow><TableCell colSpan={6} className="text-center text-sm text-muted-foreground py-6">No task results yet.</TableCell></TableRow>
              )}
              {taskResults.map((tr) => {
                const isExpanded = expandedRows.has(tr.id);
                return (
                  <Collapsible key={tr.id} asChild open={isExpanded} onOpenChange={() => toggleRow(tr.id)}>
                    <>
                      <CollapsibleTrigger asChild>
                        <TableRow className="cursor-pointer group">
                          <TableCell className="py-2 w-8"><ChevronRight className={cn("h-3 w-3 text-muted-foreground transition-transform", isExpanded && "rotate-90")} /></TableCell>
                          <TableCell className="py-2 font-mono text-xs text-foreground">{tr.taskId}</TableCell>
                          <TableCell className="py-2 text-xs text-muted-foreground">{tr.agent}</TableCell>
                          <TableCell className="py-2">
                            {tr.success ? <span className="flex items-center gap-1 text-ok text-xs"><CheckCircle className="h-3 w-3" />Success</span> : <span className="flex items-center gap-1 text-danger text-xs"><XCircle className="h-3 w-3" />Failed</span>}
                          </TableCell>
                          <TableCell className="py-2 text-right font-mono text-xs text-muted-foreground">{tr.durationMs}ms</TableCell>
                          <TableCell className="py-2 text-right font-mono text-[11px] text-muted-foreground">{new Date(tr.timestamp).toLocaleTimeString()}</TableCell>
                        </TableRow>
                      </CollapsibleTrigger>
                      <CollapsibleContent asChild>
                        <tr><td colSpan={6} className="p-0">
                          <div className="px-4 py-3 bg-muted/10 border-t border-border/30">
                            <SchemaRenderer schema={inferResultSchema(tr.result)} data={tr.result} readOnly />
                          </div>
                        </td></tr>
                      </CollapsibleContent>
                    </>
                  </Collapsible>
                );
              })}
            </TableBody>
          </Table>
        </div>
      </Card>
    </div>
  );
}

function inferResultSchema(data: unknown): any {
  if (data === null || data === undefined) return { type: "string" };
  if (typeof data === "boolean") return { type: "boolean" };
  if (typeof data === "number") return { type: "number" };
  if (typeof data === "string") return { type: "string" };
  if (Array.isArray(data)) return { type: "array", items: data.length > 0 ? inferResultSchema(data[0]) : { type: "string" } };
  if (typeof data === "object") {
    const props: Record<string, any> = {};
    for (const [k, v] of Object.entries(data as Record<string, unknown>)) props[k] = inferResultSchema(v);
    return { type: "object", properties: props };
  }
  return { type: "string" };
}

export default function OrchestrationPage() {
  return <ReactFlowProvider><OrchestrationInner /></ReactFlowProvider>;
}
