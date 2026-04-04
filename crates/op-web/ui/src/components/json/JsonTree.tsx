import { useState, useCallback } from "react";
import { cn } from "@/lib/utils";
import { ChevronRight, Copy, Check } from "lucide-react";

interface JsonTreeProps {
  data: unknown;
  name?: string;
  defaultExpanded?: boolean;
  depth?: number;
  className?: string;
}

export function JsonTree({ data, name, defaultExpanded = true, depth = 0, className }: JsonTreeProps) {
  return (
    <div className={cn("font-mono text-xs", className)}>
      <JsonNode data={data} name={name} depth={depth} defaultExpanded={defaultExpanded} />
    </div>
  );
}

function JsonNode({ data, name, depth, defaultExpanded }: { data: unknown; name?: string; depth: number; defaultExpanded: boolean }) {
  const [expanded, setExpanded] = useState(defaultExpanded && depth < 3);

  if (data === null) return <JsonLeaf name={name} value="null" color="text-muted-foreground" />;
  if (data === undefined) return <JsonLeaf name={name} value="undefined" color="text-muted-foreground" />;
  if (typeof data === "boolean") return <JsonLeaf name={name} value={String(data)} color="text-warning" />;
  if (typeof data === "number") return <JsonLeaf name={name} value={String(data)} color="text-info" />;
  if (typeof data === "string") return <JsonLeaf name={name} value={`"${data}"`} color="text-success" />;

  if (Array.isArray(data)) {
    if (data.length === 0) return <JsonLeaf name={name} value="[]" color="text-muted-foreground" />;
    return (
      <div style={{ paddingLeft: depth > 0 ? 12 : 0 }}>
        <button onClick={() => setExpanded(!expanded)} className="flex items-center gap-1 hover:text-primary py-px">
          <ChevronRight className={cn("h-3 w-3 transition-transform", expanded && "rotate-90")} />
          {name && <span className="text-foreground">{name}</span>}
          <span className="text-muted-foreground">Array[{data.length}]</span>
        </button>
        {expanded && data.map((item, i) => (
          <JsonNode key={i} data={item} name={String(i)} depth={depth + 1} defaultExpanded={defaultExpanded} />
        ))}
      </div>
    );
  }

  if (typeof data === "object") {
    const entries = Object.entries(data as Record<string, unknown>);
    if (entries.length === 0) return <JsonLeaf name={name} value="{}" color="text-muted-foreground" />;
    return (
      <div style={{ paddingLeft: depth > 0 ? 12 : 0 }}>
        <button onClick={() => setExpanded(!expanded)} className="flex items-center gap-1 hover:text-primary py-px">
          <ChevronRight className={cn("h-3 w-3 transition-transform", expanded && "rotate-90")} />
          {name && <span className="text-foreground">{name}</span>}
          <span className="text-muted-foreground">{`{${entries.length}}`}</span>
        </button>
        {expanded && entries.map(([k, v]) => (
          <JsonNode key={k} data={v} name={k} depth={depth + 1} defaultExpanded={defaultExpanded} />
        ))}
      </div>
    );
  }

  return <JsonLeaf name={name} value={String(data)} color="text-foreground" />;
}

function JsonLeaf({ name, value, color }: { name?: string; value: string; color: string }) {
  return (
    <div className="py-px" style={{ paddingLeft: 16 }}>
      {name && <span className="text-foreground">{name}: </span>}
      <span className={color}>{value}</span>
    </div>
  );
}

export function JsonCodeBlock({ data, className }: { data: unknown; className?: string }) {
  const [copied, setCopied] = useState(false);
  const json = JSON.stringify(data, null, 2);

  const copy = useCallback(() => {
    navigator.clipboard.writeText(json);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [json]);

  return (
    <div className={cn("relative rounded border bg-muted/50", className)}>
      <button onClick={copy} className="absolute right-2 top-2 p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground">
        {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
      </button>
      <pre className="p-3 text-xs font-mono overflow-auto max-h-96 text-foreground">{json}</pre>
    </div>
  );
}
