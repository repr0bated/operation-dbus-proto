/**
 * Schema-driven Generative UI renderer.
 * Maps JSON Schema types → Shadcn primitives with onChange support.
 */
import { Component, type ErrorInfo, type ReactNode, useCallback, useMemo } from "react";
import { cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Label } from "@/components/ui/label";
import type { JsonSchema } from "@/types/api";

// ── Error Boundary ──────────────────────────────────────
interface EBProps { children: ReactNode; fallback?: ReactNode }
interface EBState { hasError: boolean; error?: Error }

export class SchemaErrorBoundary extends Component<EBProps, EBState> {
  state: EBState = { hasError: false };
  static getDerivedStateFromError(error: Error) { return { hasError: true, error }; }
  componentDidCatch(error: Error, info: ErrorInfo) { console.error("[SchemaRenderer]", error, info); }
  render() {
    if (this.state.hasError) {
      return this.props.fallback ?? (
        <div className="rounded-md border border-danger/30 bg-danger/5 px-3 py-2 text-xs text-danger font-mono">
          Render error: {this.state.error?.message ?? "unknown"}
        </div>
      );
    }
    return this.props.children;
  }
}

// ── Main Component ──────────────────────────────────────
export interface SchemaRendererProps {
  schema: JsonSchema;
  data: unknown;
  onChange?: (updated: unknown) => void;
  className?: string;
  readOnly?: boolean;
  depth?: number;
  label?: string;
}

export function SchemaRenderer({ schema, data, onChange, className, readOnly = false, depth = 0, label }: SchemaRendererProps) {
  return (
    <SchemaErrorBoundary>
      <SchemaNode schema={schema} data={data} onChange={onChange} className={className} readOnly={readOnly} depth={depth} label={label} />
    </SchemaErrorBoundary>
  );
}

// ── Recursive Node ──────────────────────────────────────
function SchemaNode({ schema, data, onChange, className, readOnly, depth = 0, label }: SchemaRendererProps & { depth: number }) {
  const type = schema.type ?? (schema.enum ? "string" : schema.properties ? "object" : "unknown");

  // boolean → Switch + Badge
  if (type === "boolean") {
    return (
      <FieldRow label={label} description={schema.description} depth={depth} className={className}>
        <div className="flex items-center gap-2">
          <Switch
            checked={Boolean(data)}
            disabled={readOnly}
            onCheckedChange={(v) => onChange?.(v)}
          />
          <Badge variant={data ? "default" : "secondary"} className="text-[10px]">
            {data ? "true" : "false"}
          </Badge>
        </div>
      </FieldRow>
    );
  }

  // string with enum → Select
  if (type === "string" && schema.enum) {
    return (
      <FieldRow label={label} description={schema.description} depth={depth} className={className}>
        <Select
          value={String(data ?? "")}
          disabled={readOnly}
          onValueChange={(v) => onChange?.(v)}
        >
          <SelectTrigger className="h-8 text-xs font-mono bg-secondary border-none">
            <SelectValue placeholder="Select..." />
          </SelectTrigger>
          <SelectContent>
            {schema.enum.map((v) => (
              <SelectItem key={String(v)} value={String(v)} className="text-xs font-mono">{String(v)}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </FieldRow>
    );
  }

  // string → Input
  if (type === "string") {
    return (
      <FieldRow label={label} description={schema.description} depth={depth} className={className}>
        <Input
          type={schema.format === "password" ? "password" : "text"}
          value={String(data ?? "")}
          readOnly={readOnly}
          onChange={(e) => onChange?.(e.target.value)}
          placeholder={schema.default !== undefined ? String(schema.default) : label ?? ""}
          className="h-8 text-xs font-mono bg-secondary border-none"
        />
      </FieldRow>
    );
  }

  // number / integer
  if (type === "number" || type === "integer") {
    const numVal = Number(data ?? 0);
    const min = (schema.minimum as number | undefined) ?? 0;
    const max = (schema.maximum as number | undefined) ?? 100;
    const isPercent = max <= 100 && min >= 0;
    const hasRange = schema.minimum !== undefined || schema.maximum !== undefined;

    // If it looks like a progress/percentage metric in readOnly, show Progress
    if (readOnly && isPercent) {
      return (
        <FieldRow label={label} description={schema.description} depth={depth} className={className}>
          <div className="flex items-center gap-3">
            <Progress value={numVal} className="h-2 flex-1" />
            <span className="text-xs font-mono font-medium text-foreground w-12 text-right">{numVal.toFixed(1)}%</span>
          </div>
        </FieldRow>
      );
    }

    // Editable with range → Slider + number input
    if (hasRange && !readOnly) {
      return (
        <FieldRow label={label} description={schema.description} depth={depth} className={className}>
          <div className="flex items-center gap-3">
            <Slider
              value={[numVal]}
              min={min}
              max={max}
              step={type === "integer" ? 1 : (max - min) / 100}
              onValueChange={([v]) => onChange?.(type === "integer" ? Math.round(v) : v)}
              className="flex-1"
            />
            <span className="text-xs font-mono font-medium text-foreground w-14 text-right">{numVal}</span>
          </div>
        </FieldRow>
      );
    }

    // Default: number input
    return (
      <FieldRow label={label} description={schema.description} depth={depth} className={className}>
        <Input
          type="number"
          value={numVal}
          readOnly={readOnly}
          onChange={(e) => onChange?.(type === "integer" ? parseInt(e.target.value, 10) : parseFloat(e.target.value))}
          className="h-8 text-xs font-mono bg-secondary border-none w-28"
        />
      </FieldRow>
    );
  }

  // array → render items
  if (type === "array") {
    const arr = Array.isArray(data) ? data : [];
    return (
      <FieldRow label={label} description={schema.description} depth={depth} className={className}>
        <div className="space-y-1.5">
          {arr.length === 0 && <span className="text-[11px] text-muted-foreground italic">empty array</span>}
          {arr.map((item, i) => (
            <SchemaNode
              key={i}
              schema={schema.items ?? { type: "string" }}
              data={item}
              onChange={readOnly ? undefined : (v) => {
                const next = [...arr];
                next[i] = v;
                onChange?.(next);
              }}
              readOnly={readOnly}
              depth={depth + 1}
              label={`[${i}]`}
            />
          ))}
        </div>
      </FieldRow>
    );
  }

  // object → Card grouping
  if (type === "object" && schema.properties) {
    const obj = (data && typeof data === "object" && !Array.isArray(data)) ? data as Record<string, unknown> : {};
    const props = schema.properties;
    const required = schema.required ?? [];

    return (
      <div className={cn(
        depth > 0 && "ml-3 pl-3 border-l border-border/50",
        className,
      )}>
        {label && (
          <div className="flex items-baseline gap-2 mb-2">
            <span className="text-xs font-semibold text-foreground">{label}</span>
            {schema.title && <span className="text-[11px] text-muted-foreground">{schema.title}</span>}
          </div>
        )}
        {schema.description && !label && (
          <p className="text-[11px] text-muted-foreground mb-2">{schema.description}</p>
        )}
        <div className={cn(
          "rounded-lg border border-border bg-card/50 p-3 space-y-3",
          depth === 0 && "bg-card",
        )}>
          {schema.title && depth === 0 && (
            <div className="text-[13px] font-semibold text-foreground mb-1">{schema.title}</div>
          )}
          {Object.entries(props).map(([key, propSchema]) => (
            <SchemaNode
              key={key}
              schema={propSchema}
              data={obj[key]}
              onChange={readOnly ? undefined : (v) => {
                onChange?.({ ...obj, [key]: v });
              }}
              readOnly={readOnly}
              depth={depth + 1}
              label={`${key}${required.includes(key) ? " *" : ""}`}
            />
          ))}
        </div>
      </div>
    );
  }

  // Fallback for unknown types
  return (
    <FieldRow label={label} description={schema.description} depth={depth} className={className}>
      <pre className="font-mono text-[11px] text-muted-foreground bg-muted/30 rounded-md px-2 py-1 overflow-auto max-h-24 whitespace-pre-wrap break-all">
        {JSON.stringify(data, null, 2) ?? "null"}
      </pre>
    </FieldRow>
  );
}

// ── Field Row wrapper ───────────────────────────────────
function FieldRow({ label, description, depth, className, children }: {
  label?: string;
  description?: string;
  depth: number;
  className?: string;
  children: ReactNode;
}) {
  return (
    <div className={cn("space-y-1", depth > 0 && depth <= 1 && "", className)}>
      {label && (
        <Label className="text-[11px] font-mono font-medium text-muted-foreground uppercase tracking-wider">
          {label}
        </Label>
      )}
      {children}
      {description && (
        <p className="text-[10px] text-muted-foreground/70">{description}</p>
      )}
    </div>
  );
}
