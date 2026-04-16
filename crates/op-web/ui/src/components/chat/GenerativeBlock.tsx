/**
 * GenerativeBlock — intercepts JSON UI specification blocks inside chat messages
 * and renders them as interactive Shadcn components via SchemaRenderer.
 *
 * Detects ```json:ui ... ``` code blocks or { "$schema": "ui", ... } JSON structures.
 */
import { useState, useCallback, useMemo } from "react";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Play } from "lucide-react";
import type { JsonSchema } from "@/types/api";

interface GenerativeBlockProps {
  spec: GenerativeSpec;
  onAction?: (action: string, payload: unknown) => void;
}

export interface GenerativeSpec {
  schema: JsonSchema;
  data?: Record<string, unknown>;
  title?: string;
  actions?: Array<{ label: string; action: string; variant?: "default" | "destructive" | "outline" }>;
}

/**
 * Parse chat message content for generative UI blocks.
 * Returns an array of segments: plain text or GenerativeSpec objects.
 */
export function parseGenerativeBlocks(content: string): Array<{ type: "text"; text: string } | { type: "ui"; spec: GenerativeSpec }> {
  const segments: Array<{ type: "text"; text: string } | { type: "ui"; spec: GenerativeSpec }> = [];

  // Match ```json:ui ... ``` blocks
  const uiBlockRegex = /```json:ui\s*\n([\s\S]*?)```/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = uiBlockRegex.exec(content)) !== null) {
    if (match.index > lastIndex) {
      segments.push({ type: "text", text: content.slice(lastIndex, match.index) });
    }
    try {
      const parsed = JSON.parse(match[1]);
      if (parsed.schema) {
        segments.push({ type: "ui", spec: parsed as GenerativeSpec });
      } else {
        segments.push({ type: "text", text: match[0] });
      }
    } catch {
      segments.push({ type: "text", text: match[0] });
    }
    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < content.length) {
    const remaining = content.slice(lastIndex);
    // Also check for inline JSON UI specs
    try {
      const parsed = JSON.parse(remaining);
      if (parsed && typeof parsed === "object" && parsed["$schema"] === "ui" && parsed.schema) {
        segments.push({ type: "ui", spec: { schema: parsed.schema, data: parsed.data, title: parsed.title, actions: parsed.actions } });
        return segments;
      }
    } catch {
      // Not JSON, treat as text
    }
    if (remaining.trim()) {
      segments.push({ type: "text", text: remaining });
    }
  }

  return segments.length > 0 ? segments : [{ type: "text", text: content }];
}

export function GenerativeBlock({ spec, onAction }: GenerativeBlockProps) {
  const [formData, setFormData] = useState<unknown>(spec.data ?? {});

  const handleAction = useCallback((action: string) => {
    onAction?.(action, formData);
  }, [formData, onAction]);

  return (
    <div className="rounded-lg border border-primary/20 bg-card/80 overflow-hidden">
      {spec.title && (
        <div className="px-3 py-2 border-b border-border/50 flex items-center gap-2">
          <Badge variant="outline" className="text-[10px] border-primary/30 text-primary">UI</Badge>
          <span className="text-xs font-medium text-foreground">{spec.title}</span>
        </div>
      )}
      <div className="p-3">
        <SchemaRenderer
          schema={spec.schema}
          data={formData}
          onChange={setFormData}
        />
      </div>
      {spec.actions && spec.actions.length > 0 && (
        <div className="px-3 py-2 border-t border-border/50 flex items-center gap-2">
          {spec.actions.map((act) => (
            <Button
              key={act.action}
              size="sm"
              variant={act.variant === "destructive" ? "destructive" : act.variant === "outline" ? "outline" : "default"}
              className="h-7 text-xs gap-1.5"
              onClick={() => handleAction(act.action)}
            >
              <Play className="h-3 w-3" />
              {act.label}
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}
