/**
 * Custom React Flow node that renders a tool's input schema
 * as an auto-generated form using SchemaRenderer.
 */
import { memo, useCallback, useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { Badge } from "@/components/ui/badge";
import { Grip, X } from "lucide-react";
import { cn } from "@/lib/utils";
import type { JsonSchema } from "@/types/api";

export interface WorkflowNodeData {
  toolId: string;
  toolName: string;
  category: string;
  inputSchema: JsonSchema;
  formValues: Record<string, unknown>;
  onFormChange?: (nodeId: string, values: Record<string, unknown>) => void;
  onRemove?: (nodeId: string) => void;
  [key: string]: unknown;
}

export const WorkflowNodeCard = memo(function WorkflowNodeCard({ id, data }: NodeProps) {
  const d = data as unknown as WorkflowNodeData;
  
  const handleChange = useCallback((updated: unknown) => {
    d.onFormChange?.(id, updated as Record<string, unknown>);
  }, [id, d]);

  return (
    <div className="rounded-lg border border-border bg-card shadow-lg min-w-[260px] max-w-[320px] overflow-hidden">
      <Handle type="target" position={Position.Left} className="!bg-primary !border-primary/50 !w-3 !h-3" />

      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-border bg-muted/20">
        <Grip className="h-3 w-3 text-muted-foreground cursor-grab" />
        <div className="flex-1 min-w-0">
          <div className="text-xs font-semibold text-foreground truncate">{d.toolName}</div>
        </div>
        <Badge variant="outline" className="text-[9px] shrink-0">{d.category}</Badge>
        <button
          onClick={() => d.onRemove?.(id)}
          className="p-0.5 rounded hover:bg-danger/10 text-muted-foreground hover:text-danger transition-colors"
        >
          <X className="h-3 w-3" />
        </button>
      </div>

      {/* Schema-driven form */}
      <div className="p-3 max-h-[300px] overflow-auto nodrag nowheel">
        <SchemaRenderer
          schema={d.inputSchema}
          data={d.formValues}
          onChange={handleChange}
        />
      </div>

      <Handle type="source" position={Position.Right} className="!bg-ok !border-ok/50 !w-3 !h-3" />
    </div>
  );
});
