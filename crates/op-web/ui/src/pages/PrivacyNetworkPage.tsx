import { useState, useCallback, useMemo } from "react";
import { PageHeader } from "@/components/shell/Primitives";
import {
  ReactFlow, Background, Controls, MiniMap,
  type Node, type Edge, type NodeProps, Handle, Position,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { Slider } from "@/components/ui/slider";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { StatusDot } from "@/components/shell/Primitives";
import { useEventStore } from "@/stores/event-store";
import { cn } from "@/lib/utils";
import { Shield, Globe, Zap, Radio } from "lucide-react";

const OBF_LABELS = ["None", "Basic", "Pattern Hiding", "Advanced"] as const;

interface PNodeData {
  label: string;
  component: string;
  status: "online" | "degraded" | "offline";
  icon: string;
  config: Record<string, unknown>;
  schema: Record<string, unknown>;
  obfuscation_level?: number;
  [key: string]: unknown;
}

const ICON_MAP: Record<string, typeof Shield> = { Shield, Globe, Zap, Radio };

function PrivacyNode({ data, selected }: NodeProps<Node<PNodeData>>) {
  const Icon = (typeof data.icon === "string" ? ICON_MAP[data.icon] : data.icon) ?? Shield;
  const statusMap = { online: "ok", degraded: "warn", offline: "error" } as const;
  const busy = data.status === "degraded";

  return (
    <div className={cn(
      "rounded-lg border bg-card px-4 py-3 min-w-[160px] transition-all",
      selected ? "border-primary ring-1 ring-primary/30" : "border-border",
      busy && "animate-pulse shadow-[0_0_12px_hsl(var(--warning)/0.4)]"
    )}>
      <Handle type="target" position={Position.Left} className="!bg-primary !w-2 !h-2" />
      <Handle type="source" position={Position.Right} className="!bg-primary !w-2 !h-2" />
      <div className="flex items-center gap-2 mb-1">
        <Icon className="h-4 w-4 text-primary" />
        <span className="text-sm font-semibold text-foreground">{data.label}</span>
      </div>
      <div className="flex items-center gap-1.5">
        <StatusDot status={statusMap[data.status] ?? "offline"} />
        <span className="text-[11px] text-muted-foreground font-mono">{data.component}</span>
      </div>
      {data.obfuscation_level !== undefined && (
        <Badge variant="outline" className="mt-2 text-[10px]">Obf: {OBF_LABELS[data.obfuscation_level]}</Badge>
      )}
    </div>
  );
}

const nodeTypes = { privacy: PrivacyNode };

export default function PrivacyNetworkPage() {
  const { latestState } = useEventStore();
  const [selectedNode, setSelectedNode] = useState<Node<PNodeData> | null>(null);
  const [localConfig, setLocalConfig] = useState<Record<string, unknown>>({});

  const nodes = useMemo(() => {
    const raw = latestState["privacy.nodes"] ?? latestState["privacy:nodes"] ?? latestState["network.privacy"];
    if (Array.isArray(raw)) {
      return (raw as any[]).map((n) => ({
        ...n,
        type: n.type ?? "privacy",
        data: { ...n.data, icon: n.data?.icon ?? "Shield" },
      })) as Node<PNodeData>[];
    }
    return [] as Node<PNodeData>[];
  }, [latestState]);

  const edges = useMemo(() => {
    const raw = latestState["privacy.edges"] ?? latestState["privacy:edges"] ?? latestState["network.privacy_edges"];
    if (Array.isArray(raw)) return raw as Edge[];
    return [] as Edge[];
  }, [latestState]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    setSelectedNode(node as Node<PNodeData>);
    setLocalConfig((node as Node<PNodeData>).data.config ?? {});
  }, []);

  const handleObfChange = (val: number[]) => {
    if (!selectedNode) return;
    setLocalConfig((p) => ({ ...p, obfuscation_level: val[0] }));
  };

  return (
    <>
      <PageHeader title="Privacy Network" subtitle="Real-time topology of the privacy routing stack." />

      <div className="rounded-lg border border-border bg-card overflow-hidden" style={{ height: "calc(100vh - 180px)" }}>
        {nodes.length === 0 ? (
          <div className="flex items-center justify-center h-full text-sm text-muted-foreground">No privacy network nodes. Waiting for live data…</div>
        ) : (
          <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} onNodeClick={onNodeClick} fitView proOptions={{ hideAttribution: true }} className="[&_.react-flow__background]:!bg-background">
            <Background gap={20} size={1} className="opacity-30" />
            <Controls className="!bg-card !border-border !rounded-md [&_button]:!bg-card [&_button]:!border-border [&_button]:!text-foreground" />
            <MiniMap className="!bg-card !border-border" nodeColor="hsl(var(--primary))" />
          </ReactFlow>
        )}
      </div>

      <Sheet open={!!selectedNode} onOpenChange={(o) => !o && setSelectedNode(null)}>
        <SheetContent className="sm:max-w-lg overflow-y-auto">
          {selectedNode && (
            <>
              <SheetHeader>
                <SheetTitle className="font-mono">{selectedNode.data.label}</SheetTitle>
                <SheetDescription>{selectedNode.data.component} — click fields to edit</SheetDescription>
              </SheetHeader>
              <div className="mt-6 space-y-6">
                {selectedNode.data.obfuscation_level !== undefined && (
                  <div className="space-y-3">
                    <Label className="text-xs uppercase tracking-wider text-muted-foreground">
                      Obfuscation Level: {OBF_LABELS[(localConfig.obfuscation_level as number) ?? selectedNode.data.obfuscation_level]}
                    </Label>
                    <Slider min={0} max={3} step={1}
                      value={[(localConfig.obfuscation_level as number) ?? selectedNode.data.obfuscation_level]}
                      onValueChange={handleObfChange} />
                    <div className="flex justify-between text-[10px] text-muted-foreground font-mono">
                      {OBF_LABELS.map((l) => <span key={l}>{l}</span>)}
                    </div>
                  </div>
                )}
                <SchemaRenderer
                  schema={(selectedNode.data.schema ?? {}) as any}
                  data={localConfig}
                  onChange={(val) => setLocalConfig(val as Record<string, unknown>)}
                />
              </div>
            </>
          )}
        </SheetContent>
      </Sheet>
    </>
  );
}
