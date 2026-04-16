/**
 * State Projection Panel — renders live state_update payloads
 * using SchemaRenderer for interactive display instead of raw JSON.
 */
import { memo, useRef } from "react";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { Card } from "@/components/shell/Primitives";
import type { JsonSchema } from "@/types/api";

interface StateProjectionPanelProps {
  state: Record<string, unknown>;
  className?: string;
}

// Infer a rough schema from data for auto-rendering
function inferSchema(data: unknown): JsonSchema {
  if (data === null || data === undefined) return { type: "string" };
  if (typeof data === "boolean") return { type: "boolean" };
  if (typeof data === "number") return { type: "number", minimum: 0, maximum: 100 };
  if (typeof data === "string") return { type: "string" };
  if (Array.isArray(data)) return { type: "array", items: data.length > 0 ? inferSchema(data[0]) : { type: "string" } };
  if (typeof data === "object") {
    const props: Record<string, JsonSchema> = {};
    for (const [k, v] of Object.entries(data as Record<string, unknown>)) {
      props[k] = inferSchema(v);
    }
    return { type: "object", properties: props };
  }
  return { type: "string" };
}

// Memoize individual state key renders to prevent full-panel remount on high-frequency updates
const StateKeyRenderer = memo(function StateKeyRenderer({ keyName, value }: { keyName: string; value: unknown }) {
  const schema = inferSchema(value);
  return (
    <div className="py-2 first:pt-0 last:pb-0">
      <SchemaRenderer
        schema={schema}
        data={value}
        readOnly
        label={keyName}
      />
    </div>
  );
});

export function StateProjectionPanel({ state, className }: StateProjectionPanelProps) {
  const keys = Object.keys(state);

  if (keys.length === 0) {
    return (
      <Card title="Live State" subtitle="State projections from SSE stream." className={className}>
        <div className="text-sm text-muted-foreground mt-2 font-mono">No state updates received yet.</div>
      </Card>
    );
  }

  return (
    <Card title="Live State" subtitle="State projections rendered as live UI components." className={className}>
      <div className="space-y-3 mt-2 divide-y divide-border/30">
        {keys.map((key) => (
          <StateKeyRenderer key={key} keyName={key} value={state[key]} />
        ))}
      </div>
    </Card>
  );
}
