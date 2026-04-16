import type { JsonSchema } from "@/types/api";
import { cn } from "@/lib/utils";

interface SchemaPanelProps {
  schema: JsonSchema;
  className?: string;
}

export function SchemaPanel({ schema, className }: SchemaPanelProps) {
  return (
    <div className={cn("rounded-lg border border-border bg-card overflow-hidden", className)}>
      <div className="border-b border-border px-3 py-2">
        <span className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">Schema</span>
        {schema.title && <span className="ml-2 text-xs text-foreground font-medium">{schema.title}</span>}
      </div>
      <div className="p-3 space-y-2 max-h-[400px] overflow-auto">
        {schema.description && <p className="text-xs text-muted-foreground mb-3">{schema.description}</p>}
        {schema.properties ? (
          Object.entries(schema.properties).map(([key, prop]) => (
            <SchemaField key={key} name={key} schema={prop} required={schema.required?.includes(key)} />
          ))
        ) : (
          <pre className="font-mono text-xs text-muted-foreground">{JSON.stringify(schema, null, 2)}</pre>
        )}
      </div>
    </div>
  );
}

function SchemaField({ name, schema, required, depth = 0 }: { name: string; schema: JsonSchema; required?: boolean; depth?: number }) {
  const type = schema.type || (schema.enum ? "enum" : "unknown");
  return (
    <div className={cn("py-1.5", depth > 0 && "ml-4 border-l border-border/50 pl-3")}>
      <div className="flex items-baseline gap-2">
        <span className="font-mono text-xs font-medium text-foreground">{name}</span>
        <span className="font-mono text-[11px] text-info">{type}</span>
        {required && <span className="text-[10px] text-primary font-medium">required</span>}
        {schema.default !== undefined && <span className="text-[10px] text-muted-foreground">default: {JSON.stringify(schema.default)}</span>}
      </div>
      {schema.description && <p className="text-[11px] text-muted-foreground mt-0.5">{schema.description}</p>}
      {schema.enum && <div className="flex gap-1 mt-1 flex-wrap">{schema.enum.map((v, i) => <span key={i} className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground font-mono">{JSON.stringify(v)}</span>)}</div>}
      {schema.properties && Object.entries(schema.properties).map(([k, p]) => (
        <SchemaField key={k} name={k} schema={p} required={schema.required?.includes(k)} depth={depth + 1} />
      ))}
    </div>
  );
}
